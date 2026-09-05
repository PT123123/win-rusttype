use std::time::Instant;
use winxime_ipc::{IpcClient, IpcCommand, IpcRequest, IpcRequestData, KeyEventData};

fn stats(name: &str, v: &[f64]) {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = s.iter().sum::<f64>() / s.len() as f64;
    let p50 = s[s.len() / 2];
    let p95 = s[(s.len() as f64 * 0.95) as usize];
    println!("{name}: min={:.0}us avg={:.0}us p50={:.0}us p95={:.0}us max={:.0}us", s[0], avg, p50, p95, s[s.len() - 1]);
}

fn main() {
    let mut client = IpcClient::connect().expect("connect");
    const N: usize = 60;

    // warmup session
    let _ = client.send_request(&IpcRequest { command: IpcCommand::StartSession, session_id: 0, data: IpcRequestData::None });

    // ===== Scenario A: 空状态首码（Escape 清空）=====
    let mut first_key_esc: Vec<f64> = Vec::with_capacity(N);
    let mut follow_keys: Vec<f64> = Vec::with_capacity(N);
    for _ in 0..N {
        let _ = client.send_request(&IpcRequest { command: IpcCommand::ProcessKeyEvent, session_id: 0, data: IpcRequestData::KeyEvent(KeyEventData { keycode: 0xFF1B, modifiers: 0 }) });

        let t = Instant::now();
        let _ = client.send_request(&IpcRequest { command: IpcCommand::ProcessKeyEvent, session_id: 0, data: IpcRequestData::KeyEvent(KeyEventData { keycode: 'n' as i32, modifiers: 0 }) });
        first_key_esc.push(t.elapsed().as_micros() as f64);

        // follow-up keys n-i-h-a-o (composition active)
        for c in ['i', 'h', 'a', 'o'] {
            let t = Instant::now();
            let _ = client.send_request(&IpcRequest { command: IpcCommand::ProcessKeyEvent, session_id: 0, data: IpcRequestData::KeyEvent(KeyEventData { keycode: c as i32, modifiers: 0 }) });
            follow_keys.push(t.elapsed().as_micros() as f64);
        }
    }
    stats("A.first_key(after Esc)", &first_key_esc);
    stats("A.follow_keys(active)", &follow_keys);

    // ===== Scenario B: 空状态首码（ClearComposition 命令清空）=====
    let mut first_key_clr: Vec<f64> = Vec::with_capacity(N);
    for _ in 0..N {
        let _ = client.send_request(&IpcRequest { command: IpcCommand::ClearComposition, session_id: 0, data: IpcRequestData::None });
        let t = Instant::now();
        let _ = client.send_request(&IpcRequest { command: IpcCommand::ProcessKeyEvent, session_id: 0, data: IpcRequestData::KeyEvent(KeyEventData { keycode: 'n' as i32, modifiers: 0 }) });
        first_key_clr.push(t.elapsed().as_micros() as f64);
    }
    stats("B.first_key(after Clear)", &first_key_clr);

    let _ = client.send_request(&IpcRequest { command: IpcCommand::EndSession, session_id: 0, data: IpcRequestData::None });
}
