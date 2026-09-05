use std::time::Instant;
use winxime_ipc::{IpcClient, IpcCommand, IpcRequest, IpcRequestData, KeyEventData};

fn stats(name: &str, v: &[f64]) {
    if v.is_empty() { return; }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = s.iter().sum::<f64>() / s.len() as f64;
    let p50 = s[s.len() / 2];
    let p95 = s[(s.len() as f64 * 0.95) as usize];
    println!("{name}: min={:.0}us avg={:.0}us p50={:.0}us p95={:.0}us max={:.0}us", s[0], avg, p50, p95, s[s.len() - 1]);
}

fn key(client: &mut IpcClient, sid: u32, k: i32) -> f64 {
    let t = Instant::now();
    let _ = client.send_request(&IpcRequest {
        command: IpcCommand::ProcessKeyEvent,
        session_id: sid,
        data: IpcRequestData::KeyEvent(KeyEventData { keycode: k, modifiers: 0 }),
    });
    t.elapsed().as_micros() as f64
}

fn main() {
    let mut client = IpcClient::connect().expect("connect");
    const N: usize = 40;
    let _ = client.send_request(&IpcRequest { command: IpcCommand::StartSession, session_id: 0, data: IpcRequestData::None });

    let mut n_key: Vec<f64> = Vec::new();
    let mut i_key: Vec<f64> = Vec::new();
    let mut h_key: Vec<f64> = Vec::new();
    let mut a_key: Vec<f64> = Vec::new();
    let mut o_key: Vec<f64> = Vec::new();

    for _ in 0..N {
        let _ = client.send_request(&IpcRequest { command: IpcCommand::ClearComposition, session_id: 0, data: IpcRequestData::None });
        n_key.push(key(&mut client, 0, 'n' as i32));
        i_key.push(key(&mut client, 0, 'i' as i32));
        h_key.push(key(&mut client, 0, 'h' as i32));
        a_key.push(key(&mut client, 0, 'a' as i32));
        o_key.push(key(&mut client, 0, 'o' as i32));
    }

    stats("key n  (n)", &n_key);
    stats("key i  (ni)", &i_key);
    stats("key h  (nih)", &h_key);
    stats("key a  (niha)", &a_key);
    stats("key o  (nihao)", &o_key);

    // 对比：纯 wubi 方案？不需要，这里定位够了
    let _ = client.send_request(&IpcRequest { command: IpcCommand::EndSession, session_id: 0, data: IpcRequestData::None });
}
