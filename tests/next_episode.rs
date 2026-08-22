//! Sonraki-bölüm (n/p) geçişinde supervisor ile worker arasında deadlock olmadığını
//! doğrulayan entegrasyon testi.
//!
//! Senaryo: mpv gibi uzun yaşayan bir child process paylaşılan
//! `Arc<Mutex<Option<Child>>>` içinde tutulur. Supervisor, child çıktı mı diye
//! POLL eder (kilit bloke `wait()` ile TUTMAZ). Worker, 'n' basıldığında child'i
//! ÖLDÜRÜR ve GTK'ya "sonraki bölüm" mesajı yollar. Eğer supervisor kilit açıkken
//! blocking `wait()` yapsaydı worker kilit alamaz, child ölmez, mesaj hiç
//! gitmezdi -> bu test 5sn içinde mesajı alamazsa FAIL olur (deadlock kanıtı).

use std::process::{Child, Command};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Gerçek supervisor döngüsünün sadık kopyası (app.rs / play_candidates).
/// `playing` true ise child çıktığında başarıyla kapanmış sayılır.
fn run_supervisor(mpv_child: Arc<Mutex<Option<Child>>>) -> bool {
    let start = Instant::now();
    // Gerçek kodda IPC soketi belirince (kullanıcı 'n' basmadan çok önce) playing
    // true olur. Sahte child için "açıldı" kabul ediyoruz.
    let mut playing = true;
    loop {
        let exited = {
            let mut g = mpv_child.lock().unwrap();
            match g.as_mut().unwrap().try_wait() {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(_) => true,
            }
        };
        if exited {
            break;
        }
        if start.elapsed() > Duration::from_secs(25) && !playing {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if playing {
        if let Some(c) = mpv_child.lock().unwrap().as_mut() {
            let _ = c.wait();
        }
    }
    playing
}

#[test]
fn next_episode_no_deadlock() {
    // Uzun yaşayan sahte child (mpv gibi).
    let child = Command::new("sleep").arg("30").spawn().expect("spawn sleep");
    let mpv_child = Arc::new(Mutex::new(Some(child)));
    let (tx, rx) = mpsc::channel::<u64>(); // sonraki bölüm numarası

    let mpv_child_sup = mpv_child.clone();
    let supervisor = thread::spawn(move || run_supervisor(mpv_child_sup));

    // Worker: 'n' basıldı -> child'i öldür + sonraki bölüm mesajı yolla.
    let mpv_child_w = mpv_child.clone();
    let tx_w = tx.clone();
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        if let Some(c) = mpv_child_w.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
        let _ = tx_w.send(2u64);
    });

    // Deadlock olsaydı worker kilit alamaz, mesaj hiç gelmez, burada 5sn'de takılır.
    let got = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("DEADLOCK: worker child'i öldürüp mesajı yollayamadı");
    assert_eq!(got, 2, "sonraki bölüm mesajı yanlış");

    let played = supervisor.join().expect("supervisor panik");
    assert!(played, "supervisor child'in kapandığını algılamalı");
    worker.join().expect("worker panik");
}

#[test]
fn next_episode_prev_works() {
    let child = Command::new("sleep").arg("30").spawn().expect("spawn sleep");
    let mpv_child = Arc::new(Mutex::new(Some(child)));
    let (tx, rx) = mpsc::channel::<u64>();

    let mpv_child_sup = mpv_child.clone();
    let supervisor = thread::spawn(move || run_supervisor(mpv_child_sup));

    let mpv_child_w = mpv_child.clone();
    let tx_w = tx.clone();
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        if let Some(c) = mpv_child_w.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
        let _ = tx_w.send(1u64); // önceki bölüm (ep 2 -> 1)
    });

    let got = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("DEADLOCK: prev geçişi takıldı");
    assert_eq!(got, 1);
    assert!(supervisor.join().unwrap());
    worker.join().unwrap();
}
