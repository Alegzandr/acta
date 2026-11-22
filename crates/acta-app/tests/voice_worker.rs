//! The dictation worker end to end: the real GUI executable started with
//! `--voice-worker`, driven over its pipes with the fake engine. No model,
//! no microphone, no audio from anywhere but these generated buffers.
//! The fake engine exists only in debug builds of the executable.
#![cfg(debug_assertions)]

use std::io::BufReader;
use std::path::PathBuf;
use std::process::Child;
use std::time::{Duration, Instant};

use acta_core::voice::buffer::AudioBuf;
use acta_core::voice::engine::{Engine, EngineError, FakeStep, Lang};
use acta_core::voice::protocol::{
    EXIT_ENGINE_GONE, EngineSpec, Init, Reply, Request, WORKER_ARG, read_reply, write_request,
};
use acta_core::voice::worker::{WorkerClient, probe_gpu, worker_command};

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_Acta"))
}

fn tone(len: usize) -> Vec<f32> {
    (0..len).map(|i| (i as f32 * 0.05).sin() * 0.3).collect()
}

fn wait_exit(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn frames_round_trip_and_a_kill_mid_phrase_ends_the_stream() {
    let mut command = worker_command(&exe());
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, [WORKER_ARG], "the argument list carries no data");
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let init = Init {
        spec: EngineSpec::Fake {
            script: vec![
                FakeStep::Text("Hello Alexandre.".into()),
                FakeStep::Stall(Duration::from_secs(30), "Never.".into()),
            ],
            gpu: false,
        },
        idle_secs: 60,
    };
    write_request(&mut stdin, &Request::Init(init)).unwrap();
    write_request(&mut stdin, &Request::Start(Lang::English)).unwrap();
    write_request(
        &mut stdin,
        &Request::Phrase(AudioBuf::from_vec(tone(16_000))),
    )
    .unwrap();
    assert_eq!(read_reply(&mut stdout).unwrap(), Some(Reply::Ready));
    assert_eq!(read_reply(&mut stdout).unwrap(), Some(Reply::Started));
    assert_eq!(
        read_reply(&mut stdout).unwrap(),
        Some(Reply::Text("Hello Alexandre.".into()))
    );

    // The second phrase stalls; the worker is killed in the middle of it.
    write_request(
        &mut stdin,
        &Request::Phrase(AudioBuf::from_vec(tone(8_000))),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    child.kill().unwrap();
    let status = wait_exit(&mut child, Duration::from_secs(10)).expect("killed");
    assert!(!status.success());
    assert!(
        !matches!(read_reply(&mut stdout), Ok(Some(_))),
        "nothing after the kill"
    );
}

#[test]
fn the_client_sees_the_engine_error_and_starts_a_new_worker() {
    let spec = EngineSpec::Fake {
        script: vec![FakeStep::Text("Bonjour Alexandre.".into()), FakeStep::Exit],
        gpu: false,
    };
    let mut client = WorkerClient::new(exe(), spec);
    client.start(Lang::French).unwrap();
    let first = client.pid().unwrap();
    assert_eq!(client.feed(&tone(4_000)).unwrap(), "Bonjour Alexandre.");
    // The fake engine's Exit makes the worker exit, as a crash would.
    assert_eq!(client.feed(&tone(4_000)), Err(EngineError::Exited));
    assert_eq!(client.pid(), None);
    assert_eq!(client.feed(&tone(4_000)), Err(EngineError::Exited));
    // The next take starts a new worker, with a fresh script.
    client.start(Lang::French).unwrap();
    let second = client.pid().unwrap();
    assert_ne!(first, second);
    assert_eq!(client.feed(&tone(4_000)).unwrap(), "Bonjour Alexandre.");
    client.stop();
}

#[test]
fn the_worker_exits_with_its_code_when_the_engine_is_gone() {
    let mut command = worker_command(&exe());
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let init = Init {
        spec: EngineSpec::Fake {
            script: vec![FakeStep::Exit],
            gpu: false,
        },
        idle_secs: 60,
    };
    write_request(&mut stdin, &Request::Init(init)).unwrap();
    write_request(&mut stdin, &Request::Start(Lang::Detect)).unwrap();
    write_request(
        &mut stdin,
        &Request::Phrase(AudioBuf::from_vec(tone(1_600))),
    )
    .unwrap();
    assert_eq!(read_reply(&mut stdout).unwrap(), Some(Reply::Ready));
    assert_eq!(read_reply(&mut stdout).unwrap(), Some(Reply::Started));
    assert_eq!(
        read_reply(&mut stdout).unwrap(),
        Some(Reply::Error(EngineError::Exited))
    );
    let status = wait_exit(&mut child, Duration::from_secs(10)).expect("exited");
    assert_eq!(status.code(), Some(EXIT_ENGINE_GONE));
}

#[test]
fn an_idle_worker_exits_and_one_whose_pipe_closes_too() {
    let mut command = worker_command(&exe());
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let init = Init {
        spec: EngineSpec::Fake {
            script: vec![],
            gpu: false,
        },
        idle_secs: 1,
    };
    write_request(&mut stdin, &Request::Init(init)).unwrap();
    let started = Instant::now();
    let status = wait_exit(&mut child, Duration::from_secs(15)).expect("idle exit");
    assert!(status.success());
    assert!(started.elapsed() >= Duration::from_millis(900));
    drop(stdin);

    // Acta quitting closes the pipe: the worker ends at once.
    let mut child = worker_command(&exe()).spawn().unwrap();
    drop(child.stdin.take());
    let status = wait_exit(&mut child, Duration::from_secs(10)).expect("closed");
    assert!(status.success());
}

#[test]
fn a_missing_model_is_a_model_error() {
    let spec = EngineSpec::Whisper {
        model: std::env::temp_dir().join("acta-no-such-model-alexandre.bin"),
        vad: None,
        threads: 1,
        gpu: false,
    };
    let mut client = WorkerClient::new(exe(), spec);
    client.start(Lang::English).unwrap();
    assert_eq!(client.feed(&tone(1_600)), Err(EngineError::Model));
    // Reported at once afterwards, without starting another worker.
    assert_eq!(client.start(Lang::English), Err(EngineError::Model));
    assert_eq!(client.pid(), None);
}

#[test]
fn a_worker_that_idled_out_is_replaced_at_the_next_take() {
    let spec = EngineSpec::Fake {
        script: vec![FakeStep::Text("One.".into())],
        gpu: false,
    };
    let mut client = WorkerClient::new(exe(), spec).with_idle(Duration::from_secs(1));
    client.start(Lang::English).unwrap();
    let first = client.pid().unwrap();
    assert_eq!(client.feed(&tone(1_600)).unwrap(), "One.");
    client.stop();
    std::thread::sleep(Duration::from_millis(1_500));
    client.start(Lang::English).unwrap();
    assert_ne!(client.pid().unwrap(), first);
    assert_eq!(client.feed(&tone(1_600)).unwrap(), "One.");
}

#[test]
fn release_ends_the_worker_and_the_next_take_starts_one() {
    let spec = EngineSpec::Fake {
        script: vec![FakeStep::Text("One.".into())],
        gpu: false,
    };
    let mut client = WorkerClient::new(exe(), spec);
    client.start(Lang::English).unwrap();
    assert_eq!(client.feed(&tone(1_600)).unwrap(), "One.");
    client.release();
    assert_eq!(client.pid(), None, "the model file is free");
    client.start(Lang::English).unwrap();
    assert_eq!(client.feed(&tone(1_600)).unwrap(), "One.");
}

#[test]
fn a_worker_whose_graphics_card_fails_falls_back_to_the_processor() {
    // The fake engine "on the graphics card" ends its worker as a broken
    // driver would; the client decodes the same phrase on the processor.
    let spec = EngineSpec::Fake {
        script: vec![
            FakeStep::Text("Bonjour Alexandre.".into()),
            FakeStep::Text("Encore.".into()),
        ],
        gpu: true,
    };
    let mut client = WorkerClient::new(exe(), spec);
    assert!(client.gpu());
    client.start(Lang::French).unwrap();
    let first = client.pid().unwrap();
    assert_eq!(client.feed(&tone(4_000)).unwrap(), "Bonjour Alexandre.");
    assert!(!client.gpu(), "the processor from now on");
    let cpu = client.pid().unwrap();
    assert_ne!(cpu, first);
    assert_eq!(client.feed(&tone(4_000)).unwrap(), "Encore.");
    client.stop();
    // The next take stays on the processor, in the same worker.
    client.start(Lang::French).unwrap();
    assert_eq!(client.pid(), Some(cpu));
    assert!(!client.gpu());
    assert_eq!(client.feed(&tone(4_000)).unwrap(), "phrase 3");
}

#[test]
fn a_missing_model_on_the_graphics_card_is_still_a_model_error() {
    let spec = EngineSpec::Whisper {
        model: std::env::temp_dir().join("acta-no-such-model-alexandre.bin"),
        vad: None,
        threads: 1,
        gpu: true,
    };
    let mut client = WorkerClient::new(exe(), spec);
    client.start(Lang::English).unwrap();
    // Tried on the graphics card, then on the processor.
    assert_eq!(client.feed(&tone(1_600)), Err(EngineError::Model));
    assert!(!client.gpu());
    assert_eq!(client.start(Lang::English), Err(EngineError::Model));
    assert_eq!(client.pid(), None);
}

#[test]
fn the_probe_answers_and_its_worker_ends() {
    // Whatever this machine has (CI runners have no graphics card): an
    // answer, quickly, from a worker that is gone afterwards.
    let started = Instant::now();
    let memory = probe_gpu(&exe(), Duration::from_secs(60));
    eprintln!("graphics card memory: {memory:?}");
    assert!(memory.is_some(), "the worker answered the probe");
    assert!(started.elapsed() < Duration::from_secs(60));
}
