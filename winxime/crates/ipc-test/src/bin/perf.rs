use std::time::Instant;
use winxime_ipc::{IpcClient, IpcCommand, IpcRequest, IpcRequestData, KeyEventData};

fn main() {
    let mut client = match IpcClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("connect failed: {:?}", e);
            std::process::exit(1);
        }
    };

    // warmup: start session + one key + end
    let warm = IpcRequest { command: IpcCommand::StartSession, session_id: 0, data: IpcRequestData::None };
    let _ = client.send_request(&warm);
    let warmk = IpcRequest {
        command: IpcCommand::ProcessKeyEvent,
        session_id: 0,
        data: IpcRequestData::KeyEvent(KeyEventData { keycode: 'n' as i32, modifiers: 0 }),
    };
    let _ = client.send_request(&warmk);
    let _ = client.send_request(&IpcRequest { command: IpcCommand::EndSession, session_id: 0, data: IpcRequestData::None });

    const N: usize = 100;
    let mut start_lat: Vec<f64> = Vec::with_capacity(N);
    let mut key_lat: Vec<f64> = Vec::with_capacity(N);

    for _ in 0..N {
        // 1. StartSession latency
        let t0 = Instant::now();
        let req = IpcRequest { command: IpcCommand::StartSession, session_id: 0, data: IpcRequestData::None };
        let resp = client.send_request(&req).expect("start");
        start_lat.push(t0.elapsed().as_micros() as f64);
        let sid = resp.session_id;

        // 1.5 Clear composition (Escape) to get a clean state
        let esc = IpcRequest {
            command: IpcCommand::ProcessKeyEvent,
            session_id: sid,
            data: IpcRequestData::KeyEvent(KeyEventData { keycode: 0xFF1B, modifiers: 0 }),
        };
        let _ = client.send_request(&esc);

        // 2. First key (n) latency — 首码
        let t1 = Instant::now();
        let req = IpcRequest {
            command: IpcCommand::ProcessKeyEvent,
            session_id: sid,
            data: IpcRequestData::KeyEvent(KeyEventData { keycode: 'n' as i32, modifiers: 0 }),
        };
        let resp = client.send_request(&req).expect("key");
        key_lat.push(t1.elapsed().as_micros() as f64);
        let ctx = resp.context.as_ref().unwrap();
        if ctx.preedit.str != "n" {
            eprintln!("note: preedit={:?} success={} (iter {})", ctx.preedit.str, resp.success, start_lat.len());
        }

        // 3. EndSession
        let _ = client.send_request(&IpcRequest { command: IpcCommand::EndSession, session_id: sid, data: IpcRequestData::None });
    }

    fn stats(name: &str, v: &[f64]) {
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg = s.iter().sum::<f64>() / s.len() as f64;
        let p50 = s[s.len() / 2];
        let p95 = s[(s.len() as f64 * 0.95) as usize];
        println!("{name}: min={:.0}us avg={:.0}us p50={:.0}us p95={:.0}us max={:.0}us", s[0], avg, p50, p95, s[s.len() - 1]);
    }

    stats("StartSession", &start_lat);
    stats("FirstKey('n')", &key_lat);
}
