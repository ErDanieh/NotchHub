//! Prints what the watcher sees; used with tests/mock_player.py.
#[tokio::main]
async fn main() {
    let m = notchhub_mpris::spawn();
    let mut rx = m.tracks.clone();
    let cmds = m.commands.clone();
    let mut sent = false;
    for _ in 0..8 {
        if tokio::time::timeout(std::time::Duration::from_secs(2), rx.changed()).await.is_ok() {
            let t = rx.borrow().clone();
            println!("TRACK {:?}", t.as_ref().map(|t| (&t.title, &t.artist, t.duration, t.playing, t.app.clone())));
            if !sent && t.is_some() {
                sent = true;
                cmds.send(notchhub_mpris::Command::PlayPause).unwrap();
                cmds.send(notchhub_mpris::Command::Seek(120.0)).unwrap();
            }
        }
    }
}
