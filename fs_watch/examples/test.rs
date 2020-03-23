
use std::time::Duration;
use fs_watch::*;

fn main() {
    let mut watch = PollWatch::new()
        .expect("Couldn't create watch!")
        .with_interval(Duration::from_secs(1));

    watch.watch("C:/TMP/szavak", Recursion::Recursive);

    loop {
        while let Some(event) = watch.poll_event() {
            println!("{:?}", event);
        }
    }
}
