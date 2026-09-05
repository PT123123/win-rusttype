use winxime_ipc::{
    IpcClient, IpcRequest, IpcRequestData, KeyEventData, IpcCommand,
};

fn main() {
    let mut client = match IpcClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("connect failed: {:?}", e);
            std::process::exit(1);
        }
    };
    println!("connected");

    // StartSession
    let start = IpcRequest {
        command: IpcCommand::StartSession,
        session_id: 0,
        data: IpcRequestData::None,
    };
    let resp = client.send_request(&start).expect("start session");
    let session_id = resp.session_id;
    println!("session: {}", session_id);

    // Type "nihao" using X11 keysyms
    let keys = ['n', 'i', 'h', 'a', 'o']
        .iter()
        .map(|c| *c as u32 as i32)
        .collect::<Vec<_>>();

    for k in keys {
        let req = IpcRequest {
            command: IpcCommand::ProcessKeyEvent,
            session_id,
            data: IpcRequestData::KeyEvent(KeyEventData {
                keycode: k,
                modifiers: 0,
            }),
        };
        let resp = client.send_request(&req).expect("key event");
        let ctx = resp.context.as_ref().expect("context");
        print!("key={} -> preedit={:?} commit={:?}", k as u8 as char, ctx.preedit.str, ctx.commit);
        if !ctx.candidates.candies.is_empty() {
            print!(" cands=[");
            for (i, c) in ctx.candidates.candies.iter().enumerate() {
                if i > 0 { print!(", "); }
                print!("{}", c.str);
            }
            print!("]");
        }
        println!();
    }

    // Select candidate 1 (index 0)
    let sel = IpcRequest {
        command: IpcCommand::SelectCandidate,
        session_id,
        data: IpcRequestData::SelectIndex(0),
    };
    let resp = client.send_request(&sel).expect("select");
    let ctx = resp.context.as_ref().expect("context");
    println!("after select: preedit={:?} commit={:?}", ctx.preedit.str, ctx.commit);

    // End session
    let end = IpcRequest {
        command: IpcCommand::EndSession,
        session_id,
        data: IpcRequestData::None,
    };
    let _ = client.send_request(&end);
    println!("done");
}
