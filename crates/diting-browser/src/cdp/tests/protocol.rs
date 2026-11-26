//! The frame and the error codes: what a command that is not a good one is
//! answered. Every row below was sent to Chrome 158 and to this bridge, on
//! a raw socket, and answered alike — the walk over a frame, the codes,
//! the words — except the answers that are this bridge's own on purpose,
//! each named where it is pinned:
//!
//! * a frame that is not JSON is `-32700` in plainer words than the ones
//!   Chrome's parser writes;
//! * a key written twice in one frame is read, the last value kept, where
//!   Chrome refuses the frame;
//! * a `-32602` carries no `data`. Chrome adds a note of where its reader
//!   stopped; the readers here have no position to name;
//! * Chrome refuses `[]` for `params` on three commands and takes it on
//!   the rest. Those three take it here too.
//!
//! Then what each command's parameters are refused for, and the commands
//! Chrome has that this bridge does not answer.
use super::*;

/// Chrome's words, as it writes them.
const ID: &str = "Message must have integer 'id' property";
const SESSION: &str = "Message may have string 'sessionId' property";
const METHOD: &str = "Message must have string 'method' property";
const PARAMS: &str = "Message may have object 'params' property";
const OTHER: &str = "Message has property other than 'id', 'method', 'sessionId', 'params'";
const OBJECT: &str = "Message must be an object";
const NOT_FOUND: &str = "'NoSuch.method' wasn't found";
const NO_SESSION: &str = "Session with given id not found.";
const INVALID_PARAMS: &str = "Invalid parameters";
/// This bridge's own.
const NOT_JSON: &str = "Message is not valid JSON";

/// `(frame, the id its answer carries, its error)`: what Chrome answers to
/// each. `None` for the id: the answer carries none. `None` for the error:
/// the command ran.
type Frame = (&'static str, Option<i64>, Option<(i64, &'static str)>);

const FRAMES: &[Frame] = &[
    // Not JSON. Chrome's words here are its parser's; these are the bridge's own
    (r#"{"#, None, Some((-32700, NOT_JSON))),     // not json
    (r#"hello"#, None, Some((-32700, NOT_JSON))), // garbage
    (r#""#, None, Some((-32700, NOT_JSON))),      // empty
    (
        r#"{"id":17,"method":"NoSuch.method"} x"#,
        None,
        Some((-32700, NOT_JSON)),
    ), // trailing text
    (
        r#"{"id":18,"method":"NoSuch.method"}{"id":19,"method":"NoSuch.method"}"#,
        None,
        Some((-32700, NOT_JSON)),
    ), // two frames
    (
        r#"{"id":01,"method":"NoSuch.method"}"#,
        None,
        Some((-32700, NOT_JSON)),
    ), // id 01
    (
        "\u{feff}{\"id\":6,\"method\":\"NoSuch.method\"}",
        None,
        Some((-32700, NOT_JSON)),
    ), // bom
    // No object
    (r#"[]"#, None, Some((-32600, OBJECT))),   // array
    (r#"5"#, None, Some((-32600, OBJECT))),    // number
    (r#""x""#, None, Some((-32600, OBJECT))),  // string
    (r#"null"#, None, Some((-32600, OBJECT))), // null
    (r#"true"#, None, Some((-32600, OBJECT))), // true
    // The `id`: no whole number that fits an `int32_t`
    (r#"{}"#, None, Some((-32600, ID))), // {}
    (
        r#"{"id":"1","method":"Browser.getVersion"}"#,
        None,
        Some((-32600, ID)),
    ), // id string
    (
        r#"{"id":1.5,"method":"Browser.getVersion"}"#,
        None,
        Some((-32600, ID)),
    ), // id 1.5
    (
        r#"{"id":null,"method":"Browser.getVersion"}"#,
        None,
        Some((-32600, ID)),
    ), // id null
    (
        r#"{"id":true,"method":"Browser.getVersion"}"#,
        None,
        Some((-32600, ID)),
    ), // id true
    (
        r#"{"id":2147483648,"method":"Browser.getVersion"}"#,
        None,
        Some((-32600, ID)),
    ), // id 2^31
    (
        r#"{"id":-2147483649,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id -2^31-1
    (
        r#"{"id":1e10,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id 1e10
    (
        r#"{"id":{},"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id object
    (
        r#"{"id":[1],"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id array
    (
        r#"{"id":4294967296,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id 4294967296
    (
        r#"{"id":12345678901234567890,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id big int
    (
        r#"{"id":2.5e0,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, ID)),
    ), // id 2.5e0
    (
        r#"{"method":"NoSuch.method","params":{},"id":"x"}"#,
        None,
        Some((-32600, ID)),
    ), // bad id after good
    // The `id`: a good one comes back spelled as a whole number
    (r#"{"id":2.0,"method":"Browser.getVersion"}"#, Some(2), None), // id 2.0
    (r#"{"id":1e0,"method":"Browser.getVersion"}"#, Some(1), None), // id 1e0
    (
        r#"{"id":2147483647,"method":"NoSuch.method"}"#,
        Some(2147483647),
        Some((-32601, NOT_FOUND)),
    ), // id 2^31-1
    (
        r#"{"id":-2147483648,"method":"NoSuch.method"}"#,
        Some(-2147483648),
        Some((-32601, NOT_FOUND)),
    ), // id -2^31
    (
        r#"{"id":-5,"method":"NoSuch.method"}"#,
        Some(-5),
        Some((-32601, NOT_FOUND)),
    ), // id negative
    (
        r#"{"id":0,"method":"NoSuch.method"}"#,
        Some(0),
        Some((-32601, NOT_FOUND)),
    ), // id 0
    (
        r#"{"id":-0,"method":"NoSuch.method"}"#,
        Some(0),
        Some((-32601, NOT_FOUND)),
    ), // id -0
    (
        r#"{"id":1E1,"method":"NoSuch.method"}"#,
        Some(10),
        Some((-32601, NOT_FOUND)),
    ), // id 1E1
    (
        r#"{"id":1.0000000000000000001,"method":"NoSuch.method"}"#,
        Some(1),
        Some((-32601, NOT_FOUND)),
    ), // id 1.0000000000000000001
    (
        r#"{"id":2147483647.0,"method":"NoSuch.method"}"#,
        Some(2147483647),
        Some((-32601, NOT_FOUND)),
    ), // id 2147483647.0
    (
        "  \n {\"id\":20,\"method\":\"NoSuch.method\"} \n",
        Some(20),
        Some((-32601, NOT_FOUND)),
    ), // whitespace around
    (
        r#"{"params":{},"method":"NoSuch.method","id":9}"#,
        Some(9),
        Some((-32601, NOT_FOUND)),
    ), // all good reverse
    (
        r#"{"method":"NoSuch.method","id":7}"#,
        Some(7),
        Some((-32601, NOT_FOUND)),
    ), // id first vs id last
    (
        r#"{"method":"NoSuch.method","id":2.0}"#,
        Some(2),
        Some((-32601, NOT_FOUND)),
    ), // id 2.0 last
    // `sessionId`: not a string; unknown (and looked for only once every key was found right); empty is the root level
    (
        r#"{"id":5,"sessionId":1,"method":"Browser.getVersion"}"#,
        Some(5),
        Some((-32600, SESSION)),
    ), // sessionId number
    (
        r#"{"id":5,"sessionId":null,"method":"NoSuch.method"}"#,
        Some(5),
        Some((-32600, SESSION)),
    ), // sessionId null
    (
        r#"{"id":5,"sessionId":"abc","method":"NoSuch.method"}"#,
        Some(5),
        Some((-32001, NO_SESSION)),
    ), // sessionId string
    (
        r#"{"id":12,"sessionId":1,"method":1}"#,
        Some(12),
        Some((-32600, SESSION)),
    ), // session then method
    (
        r#"{"id":1,"sessionId":1,"method":1}"#,
        Some(1),
        Some((-32600, SESSION)),
    ), // session bad, method number
    (
        r#"{"id":1,"sessionId":"","method":"NoSuch.method"}"#,
        Some(1),
        Some((-32601, NOT_FOUND)),
    ), // empty sessionId
    (
        r#"{"id":5,"sessionId":"","method":"Browser.getVersion"}"#,
        Some(5),
        None,
    ), // session + method ok
    (
        r#"{"id":21,"sessionId":"abc","method":"Runtime.evaluate","params":null}"#,
        Some(21),
        Some((-32001, NO_SESSION)),
    ), // session eval null params
    (
        r#"{"id":5,"sessionId":"abc","method":"NoSuch.method","params":5}"#,
        Some(5),
        Some((-32600, PARAMS)),
    ), // unknown session, params 5
    (
        r#"{"id":5,"params":5,"sessionId":"abc","method":"NoSuch.method"}"#,
        Some(5),
        Some((-32600, PARAMS)),
    ), // params 5, unknown session
    (
        r#"{"id":5,"sessionId":"abc","bogus":1}"#,
        Some(5),
        Some((-32600, OTHER)),
    ), // unknown session, bogus
    (
        r#"{"id":5,"sessionId":"abc","method":5}"#,
        Some(5),
        Some((-32600, METHOD)),
    ), // unknown session, method 5
    (
        r#"{"id":5,"method":5,"sessionId":"abc"}"#,
        Some(5),
        Some((-32600, METHOD)),
    ), // method 5, unknown session
    (
        r#"{"id":7,"method":"NoSuch.method","sessionId":"abc"}"#,
        Some(7),
        Some((-32001, NO_SESSION)),
    ), // unknown session after
    (
        r#"{"sessionId":"abc","id":7,"method":"NoSuch.method"}"#,
        Some(7),
        Some((-32001, NO_SESSION)),
    ), // unknown session before id
    (
        r#"{"sessionId":"","id":7,"method":"NoSuch.method"}"#,
        Some(7),
        Some((-32601, NOT_FOUND)),
    ), // empty session before id
    (
        r#"{"id":1,"sessionId":"","method":"NoSuch.method","params":5}"#,
        Some(1),
        Some((-32600, PARAMS)),
    ), // empty session + session-ish
    // `method`: missing, not a string, empty (that passes the walk, and is named once every key was found right)
    (r#"{"id":6,"params":{}}"#, Some(6), Some((-32600, METHOD))), // no method
    (r#"{"id":7,"method":1}"#, Some(7), Some((-32600, METHOD))),  // method number
    (r#"{"id":7,"method":null}"#, Some(7), Some((-32600, METHOD))), // method null
    (r#"{"id":7,"method":""}"#, Some(7), Some((-32600, METHOD))), // method empty
    (r#"{"id":1,"params":{}}"#, Some(1), Some((-32600, METHOD))), // params ok, no method
    (
        r#"{"id":1,"method":"","params":5}"#,
        Some(1),
        Some((-32600, PARAMS)),
    ), // empty method, params bad
    (
        r#"{"id":1,"method":"","x":1}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // empty method, other prop
    (
        r#"{"id":1,"sessionId":1,"method":""}"#,
        Some(1),
        Some((-32600, SESSION)),
    ), // empty method, session bad
    (
        r#"{"id":4,"method":" NoSuch.method"}"#,
        Some(4),
        Some((-32601, "' NoSuch.method' wasn't found")),
    ), // method with space
    (
        r#"{"id":4,"method":"browser.getversion"}"#,
        Some(4),
        Some((-32601, "'browser.getversion' wasn't found")),
    ), // method lowercase
    (
        r#"{"id":4,"method":"Browser.get\u0056ersion"}"#,
        Some(4),
        None,
    ), // unicode escape method
    (
        r#"{"id":7,"method":"No\u0000such"}"#,
        Some(7),
        Some((-32601, "'No\u{0000}such' wasn't found")),
    ), // nul in method
    // `params`: none of object, array, `null`
    (
        r#"{"id":8,"method":"Runtime.evaluate","params":5}"#,
        Some(8),
        Some((-32600, PARAMS)),
    ), // params number
    (
        r#"{"id":8,"method":"Runtime.evaluate","params":"x"}"#,
        Some(8),
        Some((-32600, PARAMS)),
    ), // params string
    (
        r#"{"id":8,"method":"Runtime.evaluate","params":true}"#,
        Some(8),
        Some((-32600, PARAMS)),
    ), // params true
    (
        r#"{"id":9,"method":"Browser.getVersion","params":null}"#,
        Some(9),
        None,
    ), // params null
    (
        r#"{"id":10,"method":"Browser.getVersion","params":[]}"#,
        Some(10),
        None,
    ), // params array
    (
        r#"{"id":3,"method":"Browser.getVersion","params":null}"#,
        Some(3),
        None,
    ), // ok frame, params null last
    // Any other key
    (
        r#"{"id":11,"method":"Browser.getVersion","x":1}"#,
        Some(11),
        Some((-32600, OTHER)),
    ), // other property
    (
        r#"{"id":11,"method":"NoSuch.method","x":1}"#,
        Some(11),
        Some((-32600, OTHER)),
    ), // other prop nosuch
    (
        r#"{"id":16,"error":{},"method":"NoSuch.method"}"#,
        Some(16),
        Some((-32600, OTHER)),
    ), // error key
    (
        r#"{"id":16,"result":{},"method":"NoSuch.method"}"#,
        Some(16),
        Some((-32600, OTHER)),
    ), // result key
    (
        r#"{"method":"NoSuch.method","x":1}"#,
        None,
        Some((-32600, OTHER)),
    ), // method + other prop
    (
        r#"{"id":1,"method":"Browser.getVersion","params":null,"x":1}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // params null, other prop
    (
        r#"{"id":1,"bogus":1,"alsobogus":2}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // two bogus
    (
        r#"{"id":1,"Method":"NoSuch.method"}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // upper case key
    // No `id` at all: answered, not taken for a notification
    (
        r#"{"method":"Browser.getVersion"}"#,
        None,
        Some((-32600, ID)),
    ), // no id
    (r#"{"method":"NoSuch.method"}"#, None, Some((-32600, ID))), // no id nosuch
    (r#"{"method":1}"#, None, Some((-32600, METHOD))),           // method number, no id
    (r#"{"params":5}"#, None, Some((-32600, PARAMS))),           // params bad, no id
    (r#"{"sessionId":1}"#, None, Some((-32600, SESSION))),       // session bad, no id
    (
        r#"{"sessionId":1,"method":"NoSuch.method","params":5,"x":1}"#,
        None,
        Some((-32600, SESSION)),
    ), // all bad, no id
    (r#"{"id":null,"method":1}"#, None, Some((-32600, ID))),     // id null, method number
    (
        r#"{"id":"a","method":"NoSuch.method","params":5,"x":1}"#,
        None,
        Some((-32600, ID)),
    ), // id bad, all bad
    (r#"{"method":""}"#, None, Some((-32600, ID))),              // empty method only
    (r#"{"method":"","params":5}"#, None, Some((-32600, PARAMS))), // empty method, no id, params 5
    (r#"{"params":{}}"#, None, Some((-32600, ID))),              // params ok, no id, no method
    (r#"{"method":"Page.enable"}"#, None, Some((-32600, ID))),   // page enable no id
    (
        r#"{"method":"Runtime.evaluate","params":{"expression":"1"}}"#,
        None,
        Some((-32600, ID)),
    ), // runtime eval no id
    // The keys are checked in the order they were written, and the answer carries the `id` only when the walk had already read it
    (
        r#"{"id":13,"method":1,"params":5}"#,
        Some(13),
        Some((-32600, METHOD)),
    ), // method then params
    (
        r#"{"id":14,"method":"NoSuch.method","params":5,"zzz":1}"#,
        Some(14),
        Some((-32600, PARAMS)),
    ), // params then other
    (
        r#"{"id":15,"method":1,"params":{},"zzz":1}"#,
        Some(15),
        Some((-32600, METHOD)),
    ), // other then method
    (
        r#"{"method":"NoSuch.method","params":5}"#,
        None,
        Some((-32600, PARAMS)),
    ), // method + params bad
    (
        r#"{"id":"a","sessionId":1,"x":2}"#,
        None,
        Some((-32600, ID)),
    ), // id bad first
    (
        r#"{"id":1,"method":5,"params":5}"#,
        Some(1),
        Some((-32600, METHOD)),
    ), // method 5 then params 5
    (
        r#"{"id":1,"params":5,"method":5}"#,
        Some(1),
        Some((-32600, PARAMS)),
    ), // params 5 then method 5
    (
        r#"{"id":1,"bogus":1,"params":5}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // bogus then params 5
    (
        r#"{"id":1,"params":5,"bogus":1}"#,
        Some(1),
        Some((-32600, PARAMS)),
    ), // params 5 then bogus
    (
        r#"{"id":1,"method":"NoSuch.method","bogus":1,"params":5}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // ok method, bogus, params 5
    (
        r#"{"id":1,"method":"NoSuch.method","params":5,"bogus":1}"#,
        Some(1),
        Some((-32600, PARAMS)),
    ), // ok method, params 5, bogus
    (
        r#"{"id":1,"method":5,"sessionId":5}"#,
        Some(1),
        Some((-32600, METHOD)),
    ), // method 5 then session 5
    (
        r#"{"id":1,"sessionId":5,"method":5}"#,
        Some(1),
        Some((-32600, SESSION)),
    ), // session 5 then method 5
    (
        r#"{"id":1,"params":5,"sessionId":5}"#,
        Some(1),
        Some((-32600, PARAMS)),
    ), // params 5 then session 5
    (
        r#"{"id":1,"sessionId":5,"params":5}"#,
        Some(1),
        Some((-32600, SESSION)),
    ), // session 5 then params 5
    (
        r#"{"id":1,"bogus":1,"sessionId":5}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // bogus, session 5
    (
        r#"{"id":1,"bogus":1,"method":5}"#,
        Some(1),
        Some((-32600, OTHER)),
    ), // bogus, method 5
    (
        r#"{"sessionId":1,"id":"x","method":"NoSuch.method"}"#,
        None,
        Some((-32600, SESSION)),
    ), // session 1 first, id bad
    (
        r#"{"params":5,"id":"x","method":"NoSuch.method"}"#,
        None,
        Some((-32600, PARAMS)),
    ), // params 5 first, id bad
    (
        r#"{"bogus":1,"id":"x","method":"NoSuch.method"}"#,
        None,
        Some((-32600, OTHER)),
    ), // bogus first, id bad
    (r#"{"method":5,"id":"x"}"#, None, Some((-32600, METHOD))), // method 5 first, id bad
    (
        r#"{"params":5,"id":7,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, PARAMS)),
    ), // params 5 before id
    (r#"{"method":5,"id":7}"#, None, Some((-32600, METHOD))),   // method 5 before id
    (
        r#"{"sessionId":5,"id":7,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, SESSION)),
    ), // session 5 before id
    (
        r#"{"bogus":1,"id":7,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, OTHER)),
    ), // bogus before id
    (
        r#"{"params":5,"id":2.0,"method":"NoSuch.method"}"#,
        None,
        Some((-32600, PARAMS)),
    ), // params 5 before id 2.0
    (
        r#"{"params":5,"id":"x","method":"NoSuch.method"}"#,
        None,
        Some((-32600, PARAMS)),
    ), // params 5 before bad id
];

/// `(frame, the id its answer carries, what the browser's socket answers,
/// what a page's does)`:
/// the frames that name a page command. On the browser's socket, with no
/// session named, there are none.
type PageCommand = (
    &'static str,
    Option<i64>,
    Option<(i64, &'static str)>,
    Option<(i64, &'static str)>,
);

const PAGE_COMMANDS: &[PageCommand] = &[
    (
        r#"{"id":10,"method":"Runtime.evaluate","params":[]}"#,
        Some(10),
        Some((-32601, "'Runtime.evaluate' wasn't found")),
        Some((-32602, INVALID_PARAMS)),
    ), // params array eval
    (
        r#"{"id":2,"method":"Page.enable","params":null}"#,
        Some(2),
        Some((-32601, "'Page.enable' wasn't found")),
        None,
    ), // Page.enable null
];

/// The answer is the row and nothing more: no `sessionId`, and an error
/// with its code and its message.
fn expect_frame(text: &str, id: Option<i64>, error: Option<(i64, &str)>, answer: &Value) {
    let mut expected = json!({});
    if let Some(id) = id {
        expected["id"] = json!(id);
    }
    if let Some((code, message)) = error {
        expected["error"] = json!({ "code": code, "message": message });
    }
    let mut rest = answer.clone();
    if error.is_none() {
        assert!(rest.get("result").is_some(), "{text}: {answer}");
        rest.as_object_mut().expect("an object").remove("result");
    }
    assert_eq!(rest, expected, "{text}");
}

#[tokio::test]
async fn a_frame_is_read_as_chrome_reads_one() {
    let _guard = CDP_LOCK.lock().await;
    let (mut browser, addr) = connect(axum::Router::new()).await;
    let (target, _) = owned_page(&mut browser, 1).await;
    let mut page = page_socket(addr, &target).await;
    for socket in [&mut browser, &mut page] {
        for &(text, id, error) in FRAMES {
            let answer = socket
                .raw(text)
                .await
                .unwrap_or_else(|| panic!("nothing answered {text:?}"));
            expect_frame(text, id, error, &answer);
        }
    }
}

#[tokio::test]
async fn a_page_command_is_no_command_of_the_browser_socket() {
    let _guard = CDP_LOCK.lock().await;
    let (mut browser, addr) = connect(axum::Router::new()).await;
    let (target, _) = owned_page(&mut browser, 1).await;
    let mut page = page_socket(addr, &target).await;
    for &(text, id, on_browser, on_page) in PAGE_COMMANDS {
        let answer = browser.raw(text).await.expect("answered");
        expect_frame(text, id, on_browser, &answer);
        let answer = page.raw(text).await.expect("answered");
        expect_frame(text, id, on_page, &answer);
    }
}

/// A key written twice in one frame. Chrome will not read the frame
/// (`-32700`, and where the key repeats). This reader keeps the last value
/// at the top of the frame; in `params` Chrome's own reader does the same.
#[tokio::test]
async fn a_key_written_twice_keeps_its_last_value() {
    let _guard = CDP_LOCK.lock().await;
    let (mut client, _) = connect(axum::Router::new()).await;
    for (text, id) in [
        (r#"{"id":2,"id":3,"method":"NoSuch.method"}"#, 3),
        (r#"{"id":9,"id":9,"method":"NoSuch.method"}"#, 9),
        (
            r#"{"id":8,"method":"NoSuch.method","params":{},"params":{}}"#,
            8,
        ),
        (
            r#"{"id":12,"method":"Browser.getVersion","method":"NoSuch.method"}"#,
            12,
        ),
    ] {
        let answer = client.raw(text).await.expect("answered");
        assert_eq!(
            answer,
            json!({ "id": id, "error": { "code": -32601, "message": NOT_FOUND } }),
            "{text}"
        );
    }
    let (_, session) = owned_page(&mut client, 1).await;
    for (params, value) in [
        (r#"{"expression":"1","expression":"2"}"#, json!(2)),
        (
            r#"{"expression":"3","returnByValue":false,"returnByValue":true}"#,
            json!(3),
        ),
    ] {
        let text = format!(
            r#"{{"id":50,"sessionId":"{session}","method":"Runtime.evaluate","params":{params}}}"#
        );
        let answer = client.raw(&text).await.expect("answered");
        assert_eq!(
            answer,
            json!({ "id": 50, "result": { "result": { "type": "number", "value": value, "description": value.to_string() } }, "sessionId": session }),
            "{text}"
        );
    }
}

/// A frame that does not pass the walk is refused before any session is
/// looked at, and its answer names none — on a session that is alive, too.
/// What a command on that session answers does name it.
#[tokio::test]
async fn a_refused_frame_names_no_session_even_on_a_live_one() {
    let _guard = CDP_LOCK.lock().await;
    let (mut browser, addr) = connect(axum::Router::new()).await;
    let (target, session) = open_owned_page(&mut browser, 1).await;
    let named = |rest: &str| format!(r#"{{"id":1,"sessionId":"{session}"{rest}"#);
    for (rest, word) in [
        (r#","params":5,"method":"NoSuch.method"}"#, PARAMS),
        (r#","method":5}"#, METHOD),
        (r#","bogus":1}"#, OTHER),
        (r#","method":"","params":5}"#, PARAMS),
    ] {
        let text = named(rest);
        let answer = browser.raw(&text).await.expect("answered");
        expect_frame(&text, Some(1), Some((-32600, word)), &answer);
    }
    let text = format!(r#"{{"sessionId":"{session}","id":"x","method":"NoSuch.method"}}"#);
    let answer = browser.raw(&text).await.expect("answered");
    expect_frame(&text, None, Some((-32600, ID)), &answer);

    // A command's answer names the session it ran on.
    for (rest, mut expected) in [
        (
            r#","method":"NoSuch.method"}"#,
            json!({ "id": 1, "error": { "code": -32601, "message": NOT_FOUND } }),
        ),
        (
            r#","method":"Runtime.evaluate","params":[]}"#,
            json!({ "id": 1, "error": { "code": -32602, "message": INVALID_PARAMS } }),
        ),
        (
            r#","method":"Runtime.evaluate","params":{"expression":"1"}}"#,
            json!({ "id": 1, "result": { "result": { "type": "number", "value": 1, "description": "1" } } }),
        ),
    ] {
        let text = named(rest);
        expected["sessionId"] = json!(session);
        let answer = browser.raw(&text).await.expect("answered");
        assert_eq!(answer, expected, "{text}");
    }
    // The session written first does not put it first: the walk still
    // finds every key before the command runs.
    let text = format!(
        r#"{{"sessionId":"{session}","params":{{"expression":"1"}},"method":"Runtime.evaluate","id":9}}"#
    );
    let answer = browser.raw(&text).await.expect("answered");
    assert_eq!(
        answer,
        json!({ "id": 9, "result": { "result": { "type": "number", "value": 1, "description": "1" } }, "sessionId": session })
    );

    // On a page's own websocket a session names nothing of that page's, and
    // the walk goes on as it does anywhere else.
    let mut page = page_socket(addr, &target).await;
    for (text, code, word) in [
        (
            format!(r#"{{"id":2,"sessionId":"{session}","params":5,"method":"NoSuch.method"}}"#),
            -32600,
            PARAMS,
        ),
        (
            r#"{"id":2,"sessionId":5,"method":"NoSuch.method"}"#.to_string(),
            -32600,
            SESSION,
        ),
        (
            r#"{"id":2,"sessionId":"abc","method":"Runtime.evaluate","params":{"expression":"1"}}"#
                .to_string(),
            -32001,
            NO_SESSION,
        ),
        (
            r#"{"id":2,"sessionId":"","method":"NoSuch.method"}"#.to_string(),
            -32601,
            NOT_FOUND,
        ),
        (
            r#"{"id":2,"method":"NoSuch.method"}"#.to_string(),
            -32601,
            NOT_FOUND,
        ),
        (
            r#"{"id":2,"method":"Runtime.evaluate","params":[]}"#.to_string(),
            -32602,
            INVALID_PARAMS,
        ),
    ] {
        let answer = page.raw(&text).await.expect("answered");
        expect_frame(&text, Some(2), Some((code, word)), &answer);
    }
}

/// How one spelling of `params` is answered.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Read {
    /// The command ran.
    Taken,
    /// `-32602`: the command reads its parameters, and would not read these.
    Invalid,
    /// `-32600`: the frame is refused before any command is named.
    Named,
}

/// Where a command is asked.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Level {
    /// On a page's session.
    Page,
    /// On the browser's socket, no session named.
    Browser,
}

const SPELLINGS: [&str; 5] = ["[]", "null", "5", r#"{"x":1}"#, r#""x""#];

/// `(command, where it is asked, what each spelling of params gets)`.
const READ: &[(&str, Level, [Read; 5])] = &[
    (
        "Page.disable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Runtime.enable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Runtime.disable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Network.enable",
        Level::Page,
        [
            Read::Invalid,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Network.disable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Log.enable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Log.disable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "CSS.disable",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Runtime.runIfWaitingForDebugger",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Page.bringToFront",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Page.getLayoutMetrics",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Page.getFrameTree",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Emulation.clearDeviceMetricsOverride",
        Level::Page,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Target.getTargetInfo",
        Level::Page,
        [
            Read::Invalid,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Page.setBypassCSP",
        Level::Page,
        [
            Read::Invalid,
            Read::Invalid,
            Read::Named,
            Read::Invalid,
            Read::Named,
        ],
    ),
    (
        "Page.setLifecycleEventsEnabled",
        Level::Page,
        [
            Read::Invalid,
            Read::Invalid,
            Read::Named,
            Read::Invalid,
            Read::Named,
        ],
    ),
    (
        "Runtime.evaluate",
        Level::Page,
        [
            Read::Invalid,
            Read::Invalid,
            Read::Named,
            Read::Invalid,
            Read::Named,
        ],
    ),
    (
        "Browser.getVersion",
        Level::Browser,
        [
            Read::Taken,
            Read::Taken,
            Read::Named,
            Read::Taken,
            Read::Named,
        ],
    ),
    (
        "Target.setDiscoverTargets",
        Level::Browser,
        [
            Read::Invalid,
            Read::Invalid,
            Read::Named,
            Read::Invalid,
            Read::Named,
        ],
    ),
];

#[tokio::test]
async fn params_is_read_the_same_way_everywhere() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    for &(method, level, spelled) in READ {
        for (spelling, expected) in SPELLINGS.iter().zip(spelled) {
            let params = serde_json::from_str(spelling).expect("spelling");
            let asked = match level {
                Level::Page => client.ask(method, params, Some(&session)).await,
                Level::Browser => client.ask(method, params, None).await,
            };
            let got = match asked {
                Ok(_) => Read::Taken,
                Err((-32602, message)) => {
                    assert_eq!(message, INVALID_PARAMS, "{method} {spelling}");
                    Read::Invalid
                }
                Err((-32600, message)) => {
                    assert_eq!(message, PARAMS, "{method} {spelling}");
                    Read::Named
                }
                Err((code, message)) => panic!("{method} {spelling}: {code} {message}"),
            };
            assert_eq!(got, expected, "{method} {spelling}");
        }
    }
    // Chrome refuses `[]` on these three and takes it on their sisters;
    // here all of them take it.
    for (method, level) in [
        ("Page.enable", Level::Page),
        ("DOM.enable", Level::Page),
        ("Target.getTargets", Level::Browser),
    ] {
        let asked = match level {
            Level::Page => client.ask(method, json!([]), Some(&session)).await,
            Level::Browser => client.ask(method, json!([]), None).await,
        };
        assert!(asked.is_ok(), "{method} []: {:?}", asked.err());
    }
    client
        .run(
            "Emulation.clearDeviceMetricsOverride",
            json!({}),
            Some(&session),
        )
        .await;
}

/// `(command, its parameters, the code it is answered with, the message)`.
type Row = (&'static str, Value, i64, &'static str);

/// Every row answered with its code and its message.
async fn answered_as(client: &mut Client, session: &str, rows: Vec<Row>) {
    for (method, params, code, message) in rows {
        let line = format!("{method} {params}");
        assert_eq!(
            client.ask(method, params, Some(session)).await,
            Err((code, message.to_string())),
            "{line}"
        );
    }
}

/// The parameters are read before the command is run, as Chrome reads
/// them, and a field of the wrong sort is refused without a word about
/// which field it was.
fn not_read() -> Vec<Row> {
    vec![
        (
            "Page.addScriptToEvaluateOnNewDocument",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":"1","runImmediately":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({"identifier":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Network.setBlockedURLs",
            json!({"urls":[1]}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Network.setBlockedURLs",
            json!({"urls":"x"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Network.setExtraHTTPHeaders",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Network.setExtraHTTPHeaders",
            json!({"headers":[]}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844,"deviceScaleFactor":1,"mobile":false,"screenOrientation":{"type":"portraitPrimary"}}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844,"deviceScaleFactor":1,"mobile":"yes"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390.5,"height":844,"deviceScaleFactor":1,"mobile":false}),
            -32602,
            "Invalid parameters",
        ),
        (
            "DOM.describeNode",
            json!({"backendNodeId":"4"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.handleJavaScriptDialog",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        ("Page.reload", json!([]), -32602, "Invalid parameters"),
        (
            "Page.reload",
            json!({"ignoreCache":"x"}),
            -32602,
            "Invalid parameters",
        ),
        ("Page.navigate", json!({}), -32602, "Invalid parameters"),
        (
            "Page.navigate",
            json!({"url":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.setLifecycleEventsEnabled",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Page.setLifecycleEventsEnabled",
            json!({"enabled":1}),
            -32602,
            "Invalid parameters",
        ),
        ("Page.setBypassCSP", json!({}), -32602, "Invalid parameters"),
        (
            "Emulation.setUserAgentOverride",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setUserAgentOverride",
            json!({"userAgent":"Probe/1.0","userAgentMetadata":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setUserAgentOverride",
            json!({"userAgent":"Probe/1.0","acceptLanguage":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setTimezoneOverride",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setTimezoneOverride",
            json!({"timezoneId":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setEmulatedMedia",
            json!({"media":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":1}]}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setEmulatedMedia",
            json!([]),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setTouchEmulationEnabled",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Emulation.setFocusEmulationEnabled",
            json!({"enabled":"x"}),
            -32602,
            "Invalid parameters",
        ),
        ("Runtime.evaluate", json!({}), -32602, "Invalid parameters"),
        (
            "Runtime.evaluate",
            json!({"expression":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","serializationOptions":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","awaitPromise":"yes"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","returnByValue":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","timeout":"x"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","objectGroup":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","disableBreaks":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","userGesture":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","allowUnsafeEvalBlockedByCSP":"x"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","uniqueContextId":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.callFunctionOn",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"objectId":"00000000000000000000000000000000","functionDeclaration":1}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"objectId":"00000000000000000000000000000000","functionDeclaration":"function(){}","arguments":"x"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"objectId":"00000000000000000000000000000000","functionDeclaration":"function(){}","silent":"x"}),
            -32602,
            "Invalid parameters",
        ),
        (
            "Runtime.releaseObject",
            json!({}),
            -32602,
            "Invalid parameters",
        ),
    ]
}

/// Chrome names these more closely, and so does the bridge, in its words.
fn chromes_words() -> Vec<Row> {
    vec![
        (
            "Network.setExtraHTTPHeaders",
            json!({"headers":{"X-Probe":1}}),
            -32602,
            "Invalid header value, string expected",
        ),
        (
            "Network.setExtraHTTPHeaders",
            json!({"headers":{"X Bad":"1"}}),
            -32602,
            "Invalid header name",
        ),
        (
            "Network.setExtraHTTPHeaders",
            json!({"headers":{"X-Ok":"a\r\nb"}}),
            -32602,
            "Invalid header value",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844,"deviceScaleFactor":1,"mobile":false,"screenOrientation":{"type":"sideways","angle":0}}),
            -32602,
            "Invalid screen orientation type value",
        ),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844,"deviceScaleFactor":-1,"mobile":false}),
            -32602,
            "deviceScaleFactor must be non-negative",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","serializationOptions":{"serialization":"bogus"}}),
            -32602,
            "Unknown serializationOptions.serialization value bogus",
        ),
        (
            "Emulation.setTouchEmulationEnabled",
            json!({"enabled":true,"maxTouchPoints":0}),
            -32602,
            "Touch points must be between 1 and 16",
        ),
        (
            "Emulation.setTouchEmulationEnabled",
            json!({"enabled":true,"maxTouchPoints":17}),
            -32602,
            "Touch points must be between 1 and 16",
        ),
        (
            "Emulation.setTimezoneOverride",
            json!({"timezoneId":"Mars/Olympus"}),
            -32602,
            "Invalid timezone id",
        ),
        (
            "Page.handleJavaScriptDialog",
            json!({"accept":true}),
            -32602,
            "No dialog is showing",
        ),
    ]
}

/// Read right and then refused: Chrome's code and Chrome's reason.
fn refused() -> Vec<Row> {
    vec![
        (
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({"identifier":"9"}),
            -32000,
            "Script not found",
        ),
        (
            "DOM.describeNode",
            json!({}),
            -32000,
            "Either nodeId, backendNodeId or objectId must be specified",
        ),
        (
            "DOM.resolveNode",
            json!({}),
            -32000,
            "Either nodeId or backendNodeId must be specified.",
        ),
        (
            "DOM.scrollIntoViewIfNeeded",
            json!({}),
            -32000,
            "Either nodeId, backendNodeId or objectId must be specified",
        ),
        (
            "DOM.getContentQuads",
            json!({}),
            -32000,
            "Either nodeId, backendNodeId or objectId must be specified",
        ),
        (
            "Page.navigate",
            json!({"url":"not a url"}),
            -32000,
            "Cannot navigate to invalid URL",
        ),
        (
            "Page.navigate",
            json!({"url":""}),
            -32000,
            "Cannot navigate to invalid URL",
        ),
        (
            "Runtime.evaluate",
            json!({"expression":"1","contextId":99}),
            -32000,
            "Cannot find context with specified id",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"functionDeclaration":"function(){}"}),
            -32602,
            "Either objectId or executionContextId or uniqueContextId must be specified",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"functionDeclaration":"function(){}","executionContextId":99}),
            -32000,
            "Cannot find context with specified id",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"objectId":"00000000000000000000000000000000","functionDeclaration":"function(){}"}),
            -32000,
            "Invalid remote object id",
        ),
        (
            "Runtime.callFunctionOn",
            json!({"objectId":"00000000000000000000000000000000","functionDeclaration":"("}),
            -32000,
            "Invalid remote object id",
        ),
        (
            "Runtime.releaseObject",
            json!({"objectId":"00000000000000000000000000000000"}),
            -32000,
            "Invalid remote object id",
        ),
        (
            "Emulation.setLocaleOverride",
            json!({"locale":"not a locale!"}),
            -32000,
            "Invalid locale name",
        ),
    ]
}

/// `-32000`, this bridge's own reasons: asked for what this engine does
/// not do. An empty ok would be taken for the thing done.
fn own_words() -> Vec<Row> {
    vec![
    ("Page.addScriptToEvaluateOnNewDocument", json!({"source":"1","includeCommandLineAPI":true}), -32000, "Page.addScriptToEvaluateOnNewDocument: includeCommandLineAPI is not supported, there is no command line API"),
    ("Emulation.setDeviceMetricsOverride", json!({"width":390,"height":844,"deviceScaleFactor":1,"mobile":false,"scale":2}), -32000, "Emulation.setDeviceMetricsOverride: scale 2 is not supported, screenshots are not rescaled"),
    ("Emulation.setDeviceMetricsOverride", json!({"width":0,"height":0,"deviceScaleFactor":1,"mobile":false}), -32000, "Emulation.setDeviceMetricsOverride: a width or height of 0 (follow the window) is not supported, send the size"),
    ("Page.setBypassCSP", json!({"enabled":false}), -32000, "Page.setBypassCSP: diting never enforces CSP"),
    ("Emulation.setFocusEmulationEnabled", json!({"enabled":false}), -32000, "Emulation.setFocusEmulationEnabled: a diting page always reports focus"),
    ("Security.setIgnoreCertificateErrors", json!({"ignore":true}), -32000, "Security.setIgnoreCertificateErrors: diting verifies every certificate"),
    ("Runtime.evaluate", json!({"expression":"1","objectGroup":"g","generatePreview":true,"userGesture":true,"includeCommandLineAPI":true,"replMode":true,"throwOnSideEffect":false,"timeout":1000,"disableBreaks":true,"allowUnsafeEvalBlockedByCSP":false}), -32000, "Runtime.evaluate: includeCommandLineAPI is not supported, there is no command line API"),
    ("Runtime.evaluate", json!({"expression":"1","serializationOptions":{"serialization":"deep"}}), -32000, "Runtime.evaluate: serializationOptions.serialization 'deep' is not supported"),
    ]
}

/// `(command, its parameters)`: calls Chrome takes as sent.
fn taken() -> Vec<(&'static str, Value)> {
    vec![
        ("Network.setBlockedURLs", json!({})),
        ("Network.setExtraHTTPHeaders", json!({"headers":{}})),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width":390,"height":844,"deviceScaleFactor":1,"mobile":false,"nope":1}),
        ),
        ("Page.reload", json!({})),
        ("Emulation.setLocaleOverride", json!({})),
        ("Emulation.setLocaleOverride", json!({"locale":""})),
        (
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-color-scheme","value":"dark"},{"name":"x","value":"y"}]}),
        ),
        ("Emulation.setEmulatedMedia", json!({})),
        (
            "Emulation.setUserAgentOverride",
            json!({"userAgent":"Probe/1.0"}),
        ),
        ("Emulation.setUserAgentOverride", json!({"userAgent":""})),
        (
            "Runtime.evaluate",
            json!({"expression":"1","replMode":true}),
        ),
        // The three switches stand where the engine is.
        ("Page.setBypassCSP", json!({ "enabled": true })),
        (
            "Emulation.setFocusEmulationEnabled",
            json!({ "enabled": true }),
        ),
        (
            "Security.setIgnoreCertificateErrors",
            json!({ "ignore": false }),
        ),
        // The flags the bridge refuses when true are false here, and the
        // call goes through.
        (
            "Runtime.evaluate",
            json!({ "expression": "1", "includeCommandLineAPI": false, "throwOnSideEffect": false }),
        ),
        (
            "Runtime.evaluate",
            json!({
                "expression": "1", "timeout": 1000, "disableBreaks": true, "userGesture": false,
                "generatePreview": false, "objectGroup": "g", "allowUnsafeEvalBlockedByCSP": true,
                "replMode": false, "includeCommandLineAPI": false, "throwOnSideEffect": false,
                "awaitPromise": false, "returnByValue": false, "silent": false,
            }),
        ),
    ]
}

#[tokio::test]
async fn what_chrome_would_not_read_is_invalid_parameters() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    answered_as(&mut client, &session, not_read()).await;
    // Nothing else rides on the error.
    let reply = client
        .exchange(7, "Runtime.evaluate", json!({}), Some(&session))
        .await;
    assert_eq!(
        reply["error"],
        json!({ "code": -32602, "message": INVALID_PARAMS })
    );
}

#[tokio::test]
async fn some_bad_parameters_are_named_as_chrome_names_them() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    answered_as(&mut client, &session, chromes_words()).await;
}

#[tokio::test]
async fn what_chrome_refuses_is_refused_in_its_words() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    answered_as(&mut client, &session, refused()).await;
}

#[tokio::test]
async fn what_this_engine_cannot_do_is_refused_and_says_why() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    answered_as(&mut client, &session, own_words()).await;
}

#[tokio::test]
async fn a_call_chrome_takes_is_taken() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    for (method, params) in taken() {
        let asked = client.ask(method, params, Some(&session)).await;
        assert!(asked.is_ok(), "{method}: {:?}", asked.err());
    }
    client
        .run(
            "Emulation.clearDeviceMetricsOverride",
            json!({}),
            Some(&session),
        )
        .await;
}

/// Commands Chrome has that this bridge does not answer: `-32601`, the
/// command named. Not an empty ok, and not an error about the session. One
/// leaves this list only on purpose, when it is answered.
const NOT_ANSWERED: &[(&str, Level)] = &[
    ("DOM.getDocument", Level::Page),
    ("DOM.requestNode", Level::Page),
    ("DOM.setFileInputFiles", Level::Page),
    ("DOM.highlightRect", Level::Page),
    ("DOMDebugger.getEventListeners", Level::Page),
    ("Page.setInterceptFileChooserDialog", Level::Page),
    ("Page.setFontFamilies", Level::Page),
    ("Page.addScriptToEvaluateOnLoad", Level::Page),
    ("Runtime.getProperties", Level::Page),
    ("Runtime.releaseObjectGroup", Level::Page),
    ("Runtime.runScript", Level::Page),
    ("Runtime.compileScript", Level::Page),
    ("Runtime.awaitPromise", Level::Page),
    ("Runtime.discardConsoleEntries", Level::Page),
    ("Runtime.setAsyncCallStackDepth", Level::Page),
    ("Runtime.terminateExecution", Level::Page),
    ("Runtime.getIsolateId", Level::Page),
    ("Network.setCacheDisabled", Level::Page),
    ("Fetch.enable", Level::Page),
    ("Storage.getCookies", Level::Page),
    ("Emulation.setScriptExecutionDisabled", Level::Page),
    ("Emulation.setDefaultBackgroundColorOverride", Level::Page),
    ("Inspector.enable", Level::Page),
    ("Browser.getWindowBounds", Level::Browser),
    ("Target.getBrowserContexts", Level::Browser),
];

#[tokio::test]
async fn what_this_bridge_does_not_have_is_said_by_name() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    for &(method, level) in NOT_ANSWERED {
        let asked = match level {
            Level::Page => client.ask(method, json!({}), Some(&session)).await,
            Level::Browser => client.ask(method, json!({}), None).await,
        };
        assert_eq!(
            asked,
            Err((-32601, format!("'{method}' wasn't found"))),
            "{level:?} {method}"
        );
    }
}

/// Every command handed on to the page's thread is met there, and every
/// one answered without state answers `{}`: `-32601` is for what the bridge
/// does not have, not for a route that ends nowhere.
#[tokio::test]
async fn every_routed_command_is_met_at_its_end() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    for method in super::super::PAGE_CHANNEL {
        match client.ask(method, json!({}), Some(&session)).await {
            Ok(_) => {}
            Err((code, message)) => {
                assert_ne!(code, -32601, "{method}");
                assert!(!message.contains("has no handler"), "{method}: {message}");
            }
        }
    }
    for method in super::super::STATELESS {
        assert_eq!(
            client.ask(method, json!({}), Some(&session)).await,
            Ok(json!({})),
            "{method}"
        );
    }
}

/// `/json/version` and `Browser.getVersion`: Chrome's keys, with this
/// engine named where Chrome names its own.
#[tokio::test]
async fn the_version_has_chromes_keys_and_names_this_engine() {
    let _guard = CDP_LOCK.lock().await;
    let (mut client, addr) = connect(axum::Router::new()).await;
    let version = http_get(addr, "/json/version/").await;
    let product = format!("diting/{}", env!("CARGO_PKG_VERSION"));
    assert_eq!(version["Browser"], product.as_str());
    assert_eq!(version["Protocol-Version"], "1.3");
    assert_eq!(version["V8-Version"], "diting");
    assert_eq!(version["WebKit-Version"], "537.36");
    assert_eq!(
        version["File-Path-Style"],
        if cfg!(windows) { "windows" } else { "posix" }
    );
    let user_agent = version["User-Agent"].as_str().expect("User-Agent");
    assert!(user_agent.ends_with(&product), "{user_agent}");
    let socket = version["webSocketDebuggerUrl"]
        .as_str()
        .expect("webSocketDebuggerUrl");
    assert!(
        socket.starts_with(&format!("ws://{addr}/devtools/browser/")),
        "{socket}"
    );
    let mut keys: Vec<&str> = version
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "Browser",
            "File-Path-Style",
            "Protocol-Version",
            "User-Agent",
            "V8-Version",
            "WebKit-Version",
            "webSocketDebuggerUrl",
        ]
    );

    assert_eq!(
        client
            .ask("Browser.getVersion", json!({}), None)
            .await
            .expect("Browser.getVersion"),
        json!({
            "protocolVersion": "1.3",
            "product": product,
            "revision": "0",
            "userAgent": user_agent,
            "jsVersion": "diting",
        })
    );
}

/// A build without the `screenshot` feature has nothing to draw with: the
/// call is refused, and says that, rather than answering an empty picture.
#[cfg(not(feature = "screenshot"))]
#[tokio::test]
async fn a_screenshot_that_cannot_be_taken_says_so() {
    let (_guard, mut client, session) = bridge("about:blank").await;
    assert_eq!(
        client
            .refusal("Page.captureScreenshot", json!({}), Some(&session))
            .await,
        super::super::NEEDS_SCREENSHOT
    );
}
