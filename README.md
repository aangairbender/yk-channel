# Learning MPSC channel by implementing one myself!

This is my implementation of `mpsc` unbounded channel. Highly inspired by some youtube videos and articles.

## Showcase

```rust
#[test]
fn it_works_in_different_threads() {
    let (tx, mut rx) = unbounded_channel();
    {
        let mut tx1 = tx.clone();
        std::thread::spawn(move || {
            assert_eq!(tx1.send(1), Ok(()));
        });
    }
    {
        let mut tx2 = tx.clone();
        std::thread::spawn(move || {
            assert_eq!(tx2.send(1), Ok(()));
        });
    }
    drop(tx);

    assert_eq!(rx.receive(), Some(1));
    assert_eq!(rx.receive(), Some(1));
    assert_eq!(rx.receive(), None);
}
```