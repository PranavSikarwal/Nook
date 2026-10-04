from nook_worker.titles import clean_title_text


def test_clean_title_text():
    assert clean_title_text('"Capital of France."') == "Capital of France"
    assert (
        clean_title_text("'How do checkpointers work?'") == "How do checkpointers work"
    )
    assert clean_title_text("This is a title:") == "This is a title"
    assert (
        clean_title_text("This is a very long title that exceeds the limit of words")
        == "This is a very long title"
    )
    assert clean_title_text("") == "New Chat"
