//! Agent failures retain their classification and source when converted.

use std::error::Error as _;

use spire_agent::Error;
use spire_core::ErrorKind;
use spire_core::worker::{IntoOutcome, Outcome};

#[test]
fn conversion_preserves_kind_message_and_domain_source() {
    for (error, kind) in [
        (Error::Budget("input tokens"), ErrorKind::Worker),
        (
            Error::Configuration("missing usage".into()),
            ErrorKind::Invalid,
        ),
        (Error::Answer("invalid JSON".into()), ErrorKind::Worker),
        (Error::Cache("read failed".into()), ErrorKind::Worker),
    ] {
        let message = error.to_string();
        let converted: spire_core::Error = error.into();
        assert_eq!(converted.kind(), kind);
        assert_eq!(converted.to_string(), message);
        assert!(!converted.is_retryable());
        assert!(
            converted
                .source()
                .unwrap()
                .downcast_ref::<Error>()
                .is_some()
        );
    }
}

#[test]
fn provider_source_chain_survives_conversion_and_question_mark() {
    fn operation() -> spire_core::Result<()> {
        Err(Error::provider(std::io::Error::other("provider failed")))?;
        Ok(())
    }
    let error = operation().unwrap_err();
    let agent = error.source().unwrap().downcast_ref::<Error>().unwrap();
    assert!(
        agent
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .is_some()
    );
}

#[test]
fn direct_agent_outcomes_keep_their_fatality_policy() {
    assert!(matches!(
        Error::Budget("input tokens").into_outcome(),
        Outcome::Fatal(_)
    ));
    assert!(matches!(
        Error::Configuration("missing usage".into()).into_outcome(),
        Outcome::Fatal(_)
    ));
    assert!(matches!(
        Error::Answer("invalid JSON".into()).into_outcome(),
        Outcome::Fail(_)
    ));
}
