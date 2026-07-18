//! `ShowBoard` (0x7B) — the community-board (BBS) window. Port of
//! `serverpackets/ShowBoard`. The board content is delivered as up to three
//! packets tagged `101`/`102`/`103` (the client reassembles them); see
//! [`crate::game_loop::community_board::send_cb_html`] for the chunker that is
//! the runtime side of Java's `Util.sendCBHtml`.

use commons::network::PacketWriter;

use super::opcodes;

/// The eight fixed leading strings Java's `writeImpl` always writes before the
/// content: the top nav bypasses the client binds to its board buttons. We
/// send the same set verbatim (the retail boards behind most of them are not
/// ported yet — the client just needs the strings present).
const NAV_BYPASSES: [&str; 8] = [
    "bypass _bbshome",   // top
    "bypass _bbsgetfav", // favorite
    "bypass _bbsloc",    // region
    "bypass _bbsclan",   // clan
    "bypass _bbsmemo",   // memo
    "bypass _bbsmail",   // mail
    "bypass _bbsfriends", // friends
    "bypass bbs_add_fav", // add fav.
];

/// `new ShowBoard(htmlCode, id)`: one content packet. `id` is the client's
/// reassembly tag (`"101"`/`"102"`/`"103"`, or `"1001"` for the multi-edit
/// window). A `None` chunk reproduces Java's `id + "" + null` — the
/// literal string `"null"` the client treats as an empty continuation.
pub fn show_board(id: &str, html: Option<&str>) -> Vec<u8> {
    write(1, &format!("{id}\u{0008}{}", html.unwrap_or("null")))
}

/// `new ShowBoard()`: the hide/close variant — `showBoard = 0`, empty content.
pub fn show_board_hide() -> Vec<u8> {
    write(0, "")
}

fn write(show_board: u8, content: &str) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_u8(opcodes::SHOW_BOARD);
    w.write_u8(show_board); // 1 = show community, 0 = hide
    for s in NAV_BYPASSES {
        w.write_string(s);
    }
    w.write_string(content);
    w.into_bytes()
}
