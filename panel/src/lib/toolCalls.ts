import type { DaemonEvent, ToolCall } from './types'

function parseArguments(raw: string): Record<string, unknown> {
  try {
    const parsed: unknown = JSON.parse(raw)
    return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

function toolLabel(name: string): string {
  return (name.split(':').pop() ?? name).replaceAll('_', ' ')
}

function toolTarget(call: ToolCall): string {
  const args = parseArguments(call.arguments)
  if (typeof args.url === 'string') {
    try {
      return new URL(args.url).hostname
    } catch {
      return args.url
    }
  }
  return typeof args.query === 'string' ? args.query : ''
}

export function applyToolEvent(calls: readonly ToolCall[], event: DaemonEvent): ToolCall[] {
  if (event.type === 'tool_call_started') {
    if (calls.some((call) => call.call_id === event.call_id)) return [...calls]
    return [
      ...calls,
      { call_id: event.call_id, name: event.name, arguments: event.arguments, status: 'running' },
    ]
  }
  if (event.type === 'tool_call_finished') {
    if (!calls.some((call) => call.call_id === event.call_id)) {
      return [
        ...calls,
        { call_id: event.call_id, name: '', arguments: '', status: 'finished', result: event.result },
      ]
    }
    return calls.map((call) =>
      call.call_id === event.call_id ? { ...call, status: 'finished', result: event.result } : call
    )
  }
  return [...calls]
}

export function describeToolCall(call: ToolCall, replyIsActive: boolean): string {
  const label = toolLabel(call.name)
  if (call.status === 'finished') return `Finished ${label}`.trim()
  if (!replyIsActive) return `Stopped ${label}`
  return `Running ${label} ${toolTarget(call)}`.trim()
}
