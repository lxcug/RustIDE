use crate::Editor;

fn rope_text(editor: &Editor) -> String {
    editor.rope().to_string()
}

#[test]
fn insert_backspace_delete() {
    let mut editor = Editor::empty();
    editor.insert_text("abc");
    assert_eq!(rope_text(&editor), "abc");

    editor.backspace();
    assert_eq!(rope_text(&editor), "ab");

    editor.set_cursor(0, false);
    editor.delete_forward();
    assert_eq!(rope_text(&editor), "b");
}

#[test]
fn selection_replaces_range() {
    let mut editor = Editor::empty();
    editor.insert_text("hello");

    editor.set_cursor(1, false);
    editor.set_cursor(4, true); // select "ell"
    editor.insert_text("X");
    assert_eq!(rope_text(&editor), "hXo");
    assert_eq!(editor.selection().anchor, editor.selection().cursor);
}

#[test]
fn undo_redo_roundtrip() {
    let mut editor = Editor::empty();
    editor.insert_text("hello");
    editor.set_cursor(1, false);
    editor.set_cursor(4, true);
    editor.insert_text("X");

    assert_eq!(rope_text(&editor), "hXo");
    assert!(editor.undo());
    assert_eq!(rope_text(&editor), "hello");
    assert!(editor.redo());
    assert_eq!(rope_text(&editor), "hXo");
}

#[test]
fn auto_indent_newline_basic() {
    let mut ed = Editor::from_text("    let x = 1;");
    ed.set_cursor(ed.rope().len_chars(), false);
    ed.insert_newline_auto_indent();
    assert_eq!(ed.rope().to_string(), "    let x = 1;\n    ");
}

#[test]
fn auto_indent_newline_block_braces() {
    let mut ed = Editor::from_text("{\n}");
    ed.set_cursor(1, false); // between '{' and '\n'
    ed.insert_newline_auto_indent();
    assert_eq!(ed.rope().to_string(), "{\n    \n}");
    assert_eq!(ed.selection().cursor, 1 + 1 + 4);
}

#[test]
fn move_up_down_preserves_column_when_possible() {
    let mut editor = Editor::empty();
    editor.insert_text("aa\nbbbb\ncc");

    let line1_start = editor.rope().line_to_char(1);
    editor.set_cursor(line1_start + 3, false); // line 1 col 3

    editor.move_up(false);
    assert_eq!(editor.selection().cursor, 2); // "aa" (clamped)

    editor.move_down(false);
    assert_eq!(editor.selection().cursor, line1_start + 3); // preferred column restored
}
