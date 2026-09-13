use crate::{PresenterCommand, PresenterState, PresenterTransition, ProtocolError, parse_command};
use std::fmt;
use std::io::{self, BufRead};

pub trait PresenterBackend {
    type Error: std::error::Error + Send + Sync + 'static;

    fn update(&mut self, state: PresenterState) -> Result<(), Self::Error>;
    fn shutdown(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum RuntimeError<E> {
    Input(io::Error),
    Protocol(ProtocolError),
    Backend(E),
}

impl<E: fmt::Display> fmt::Display for RuntimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => write!(formatter, "read presenter command: {error}"),
            Self::Protocol(error) => error.fmt(formatter),
            Self::Backend(error) => write!(formatter, "update presenter: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RuntimeError<E> {}

pub fn run_command_stream<R, B>(reader: R, backend: &mut B) -> Result<(), RuntimeError<B::Error>>
where
    R: BufRead,
    B: PresenterBackend,
{
    let mut state = PresenterState::default();

    for line in reader.lines() {
        let line = line.map_err(RuntimeError::Input)?;
        if line.trim().is_empty() {
            continue;
        }

        let command = parse_command(&line).map_err(RuntimeError::Protocol)?;
        let transition = state.apply(command);
        match transition {
            PresenterTransition::Updated => {
                backend.update(state).map_err(RuntimeError::Backend)?;
            }
            PresenterTransition::Shutdown => {
                backend.shutdown().map_err(RuntimeError::Backend)?;
                return Ok(());
            }
        }
    }

    backend.shutdown().map_err(RuntimeError::Backend)
}

pub fn encode_command(command: &PresenterCommand) -> Result<String, serde_json::Error> {
    serde_json::to_string(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PROTOCOL_VERSION, PresenterLayout, PresenterTheme};
    use std::convert::Infallible;

    #[derive(Default)]
    struct RecordingBackend {
        updates: Vec<PresenterState>,
        shutdowns: usize,
    }

    impl PresenterBackend for RecordingBackend {
        type Error = Infallible;

        fn update(&mut self, state: PresenterState) -> Result<(), Self::Error> {
            self.updates.push(state);
            Ok(())
        }

        fn shutdown(&mut self) -> Result<(), Self::Error> {
            self.shutdowns += 1;
            Ok(())
        }
    }

    #[test]
    fn drives_backend_until_shutdown() {
        let input = concat!(
            "{\"version\":1,\"type\":\"configure\",\"visible\":true,\"layout\":\"stacked\",\"theme\":\"system\"}\n",
            "{\"version\":1,\"type\":\"traffic\",\"up\":1024,\"down\":2048}\n",
            "{\"version\":1,\"type\":\"shutdown\"}\n",
        );
        let mut backend = RecordingBackend::default();

        run_command_stream(input.as_bytes(), &mut backend).expect("command stream");

        assert_eq!(backend.updates.len(), 2);
        assert_eq!(backend.updates[1].upload_label(), "↑ 1.00 KB/s");
        assert_eq!(backend.shutdowns, 1);
    }

    #[test]
    fn shuts_down_when_parent_closes_stdin() {
        let command = PresenterCommand::Configure {
            version: PROTOCOL_VERSION,
            visible: true,
            layout: PresenterLayout::Horizontal,
            theme: PresenterTheme::Light,
        };
        let input = format!("{}\n", encode_command(&command).expect("encoded command"));
        let mut backend = RecordingBackend::default();

        run_command_stream(input.as_bytes(), &mut backend).expect("command stream");

        assert_eq!(backend.updates.len(), 1);
        assert_eq!(backend.shutdowns, 1);
    }
}
