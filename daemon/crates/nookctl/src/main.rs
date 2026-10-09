use clap::{Parser, Subcommand};
use nook_core::config::Config;
use nook_core::protocol::{Attachment, AttachmentKind, ClientMessage, DaemonMessage};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "nookctl", about = "CLI client for Nook daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a message to the agent and stream the response
    Send {
        question: String,
        #[arg(long)]
        chat: Option<Uuid>,
        #[arg(long = "attach")]
        attachments: Vec<PathBuf>,
    },
    /// List all chats
    List,
    /// Show chat transcript
    Show { chat_id: Uuid },
    /// Delete a chat
    Delete { chat_id: Uuid },
    /// Ping the daemon
    Ping,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    nook_core::config::Config::load_env_file();
    let cli = Cli::parse();
    let socket_path = Config::default_socket_path();

    if !socket_path.exists() {
        eprintln!(
            "Daemon socket not found at {}. Is nookd running?",
            socket_path.display()
        );
        std::process::exit(1);
    }

    let stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Failed to connect to daemon at {}: {e}",
                socket_path.display()
            );
            std::process::exit(1);
        }
    };

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    match cli.command {
        Commands::Ping => {
            let req_id = Uuid::new_v4();
            let msg = ClientMessage::Ping { id: req_id };
            send_json(&mut writer, &msg).await?;

            let mut received_pong = false;
            if let Some(line) = lines.next_line().await? {
                if let Ok(DaemonMessage::Pong { .. }) = serde_json::from_str(&line) {
                    println!("pong");
                    received_pong = true;
                } else {
                    println!("{line}");
                }
            }
            if !received_pong {
                eprintln!("Error: Connection closed before pong received");
                std::process::exit(1);
            }
        }

        Commands::List => {
            let req_id = Uuid::new_v4();
            let msg = ClientMessage::ListChats { id: req_id };
            send_json(&mut writer, &msg).await?;

            let mut received = false;
            if let Some(line) = lines.next_line().await? {
                received = true;
                match serde_json::from_str::<DaemonMessage>(&line)? {
                    DaemonMessage::Chats { chats, .. } => {
                        if chats.is_empty() {
                            println!("No chats found.");
                        } else {
                            println!("{:<36}  {:<30}  UPDATED AT", "CHAT ID", "TITLE");
                            println!("{}", "-".repeat(90));
                            for c in chats {
                                println!(
                                    "{:<36}  {:<30}  {}",
                                    c.chat_id,
                                    c.title,
                                    c.updated_at.to_rfc3339()
                                );
                            }
                        }
                    }
                    DaemonMessage::Error { error, .. } => {
                        eprintln!("Error: {}", error.message);
                        std::process::exit(1);
                    }
                    _ => println!("{line}"),
                }
            }
            if !received {
                eprintln!("Error: Connection closed before receiving chat list");
                std::process::exit(1);
            }
        }

        Commands::Show { chat_id } => {
            let req_id = Uuid::new_v4();
            let msg = ClientMessage::GetChat {
                id: req_id,
                chat_id,
            };
            send_json(&mut writer, &msg).await?;

            let mut received = false;
            if let Some(line) = lines.next_line().await? {
                received = true;
                match serde_json::from_str::<DaemonMessage>(&line)? {
                    DaemonMessage::Chat {
                        title, messages, ..
                    } => {
                        println!("=== {} ({chat_id}) ===", title);
                        for m in messages {
                            println!("\n[{:?}]:", m.role);
                            println!("{}", m.text);
                            for a in m.attachments {
                                println!("  (attachment: {} [{}])", a.name, a.mime);
                            }
                        }
                    }
                    DaemonMessage::Error { error, .. } => {
                        eprintln!("Error: {}", error.message);
                        std::process::exit(1);
                    }
                    _ => println!("{line}"),
                }
            }
            if !received {
                eprintln!("Error: Connection closed before receiving chat");
                std::process::exit(1);
            }
        }

        Commands::Delete { chat_id } => {
            let req_id = Uuid::new_v4();
            let msg = ClientMessage::DeleteChat {
                id: req_id,
                chat_id,
            };
            send_json(&mut writer, &msg).await?;

            let mut received = false;
            if let Some(line) = lines.next_line().await? {
                received = true;
                match serde_json::from_str::<DaemonMessage>(&line)? {
                    DaemonMessage::Deleted { chat_id, .. } => {
                        println!("Deleted chat {chat_id}");
                    }
                    DaemonMessage::Error { error, .. } => {
                        eprintln!("Error: {}", error.message);
                        std::process::exit(1);
                    }
                    _ => println!("{line}"),
                }
            }
            if !received {
                eprintln!("Error: Connection closed before deletion confirmed");
                std::process::exit(1);
            }
        }

        Commands::Send {
            question,
            chat,
            attachments,
        } => {
            let req_id = Uuid::new_v4();
            let chat_id = chat.unwrap_or_else(Uuid::new_v4);

            let att_dir =
                nook_core::config::Config::default_attachments_dir().join(chat_id.to_string());
            if !attachments.is_empty() {
                std::fs::create_dir_all(&att_dir)?;
            }

            let mut att_items = Vec::new();
            for src_path in attachments {
                let name = src_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let size_bytes = std::fs::metadata(&src_path)
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                let ext = src_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                let (kind, mime) = match ext.as_str() {
                    "png" => (AttachmentKind::Image, "image/png".to_string()),
                    "jpg" | "jpeg" => (AttachmentKind::Image, "image/jpeg".to_string()),
                    "webp" => (AttachmentKind::Image, "image/webp".to_string()),
                    "gif" => (AttachmentKind::Image, "image/gif".to_string()),
                    "pdf" => (AttachmentKind::Pdf, "application/pdf".to_string()),
                    _ => (AttachmentKind::Text, "text/plain".to_string()),
                };

                let att_id = Uuid::new_v4();
                let dest_path = att_dir.join(format!("{att_id}-{name}"));
                std::fs::copy(&src_path, &dest_path)?;

                att_items.push(Attachment {
                    id: att_id,
                    kind,
                    name,
                    mime,
                    size_bytes,
                    path: dest_path.to_string_lossy().to_string(),
                });
            }

            let msg = ClientMessage::SendMessage {
                id: req_id,
                chat_id,
                text: question,
                attachments: att_items,
            };
            send_json(&mut writer, &msg).await?;

            use std::io::Write;
            let mut completed = false;
            while let Some(line) = lines.next_line().await? {
                if let Ok(event) = serde_json::from_str::<DaemonMessage>(&line) {
                    match event {
                        DaemonMessage::MessageStarted { .. } => {}
                        DaemonMessage::TextDelta { text, .. } => {
                            print!("{text}");
                            std::io::stdout().flush()?;
                        }
                        DaemonMessage::ToolCallStarted { name, .. } => {
                            eprintln!("\n[tool: {name}]");
                        }
                        DaemonMessage::ToolCallFinished { .. } => {}
                        DaemonMessage::MessageFinished { .. } => {
                            println!();
                            completed = true;
                            break;
                        }
                        DaemonMessage::Error { error, .. } => {
                            eprintln!("\nError: {}", error.message);
                            std::process::exit(1);
                        }
                        _ => {}
                    }
                }
            }
            if !completed {
                eprintln!("\nError: Connection closed before message completed");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

async fn send_json<W: AsyncWriteExt + Unpin, T: serde::Serialize>(
    writer: &mut W,
    val: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut json = serde_json::to_string(val)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
