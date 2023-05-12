//! Service telemetry.

#[cfg(any(feature = "logging", feature = "tracing"))]
mod scope;

#[cfg(feature = "testing")]
mod testing;

#[cfg(feature = "logging")]
pub mod log;

#[cfg(feature = "tracing")]
pub mod tracing;

pub mod settings;

use self::settings::TelemetrySettings;
use crate::utils::feature_use;
use crate::{BootstrapResult, ServiceInfo};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(feature = "testing")]
pub use self::testing::TestTelemetryScope;

feature_use!(cfg(feature = "logging"), {
    use self::log::internal::{current_log, fork_log, LogScope, SharedLog};
    use slog::Logger;
    use std::sync::Arc;
});

feature_use!(cfg(feature = "tracing"), {
    use self::tracing::internal::{create_span, current_span, SharedSpan, Span, SpanScope};

    feature_use!(cfg(feature = "testing"), {
        use self::tracing::internal::Tracer;
        use self::tracing::testing::{current_test_tracer, TestTracerScope};
    });
});

/// Wrapper for a future that provides it with [`TelemetryContext`].
pub struct WithTelemetryContext<'f, T> {
    // NOTE: we intentionally erase type here as we can get close to the type
    // length limit, adding telemetry wrappers on top causes compiler to fail in some
    // cases.
    inner: Pin<Box<dyn Future<Output = T> + Send + 'f>>,
    ctx: TelemetryContext,
}

impl<'f, T> Future for WithTelemetryContext<'f, T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let _telemetry_scope = self.ctx.scope();

        self.inner.as_mut().poll(cx)
    }
}

/// [`TODO ROCK-13`]
#[must_use]
pub struct TelemetryScope {
    #[cfg(feature = "logging")]
    _log_scope: LogScope,

    #[cfg(feature = "tracing")]
    _span_scope: Option<SpanScope>,

    // NOTE: certain tracing APIs start a new trace, so we need to scope the test tracer
    // for them to use the tracer from the test scope instead of production tracer in
    // the harness.
    #[cfg(all(feature = "tracing", feature = "testing"))]
    _test_tracer_scope: Option<TestTracerScope>,
}

/// [`TODO ROCK-13`]
#[derive(Debug, Clone)]
pub struct TelemetryContext {
    #[cfg(feature = "logging")]
    log: SharedLog,

    // NOTE: we might not have tracing root at this point
    #[cfg(feature = "tracing")]
    span: Option<SharedSpan>,

    #[cfg(all(feature = "tracing", feature = "testing"))]
    test_tracer: Option<Tracer>,
}

impl TelemetryContext {
    /// [`TODO ROCK-13`]
    pub fn current() -> Self {
        Self {
            #[cfg(feature = "logging")]
            log: current_log(),

            #[cfg(feature = "tracing")]
            span: current_span(),

            #[cfg(all(feature = "tracing", feature = "testing"))]
            test_tracer: current_test_tracer(),
        }
    }

    /// [`TODO ROCK-13`]
    pub fn scope(&self) -> TelemetryScope {
        TelemetryScope {
            #[cfg(feature = "logging")]
            _log_scope: LogScope::new(Arc::clone(&self.log)),

            #[cfg(feature = "tracing")]
            _span_scope: self.span.as_ref().cloned().map(SpanScope::new),

            #[cfg(all(feature = "tracing", feature = "testing"))]
            _test_tracer_scope: self.test_tracer.as_ref().cloned().map(TestTracerScope::new),
        }
    }

    /// [`TODO ROCK-13`]
    #[cfg(feature = "testing")]
    pub fn test() -> TestTelemetryScope {
        TestTelemetryScope::new()
    }

    /// [`TODO ROCK-13`]
    pub fn apply<'f, F>(self, fut: F) -> WithTelemetryContext<'f, F::Output>
    where
        F: Future + Send + 'f,
    {
        WithTelemetryContext {
            inner: Box::pin(fut),
            ctx: self,
        }
    }
}

#[cfg(feature = "tracing")]
impl TelemetryContext {
    /// [`TODO ROCK-13`]
    pub fn rustracing_span(&self) -> Option<parking_lot::RwLockReadGuard<Span>> {
        self.span.as_ref().map(|span| span.inner.read())
    }

    /// [`TODO ROCK-13`]
    pub fn apply_with_tracing_span<'f, F>(
        mut self,
        span_name: &'static str,
        fut: F,
    ) -> WithTelemetryContext<'f, F::Output>
    where
        F: Future + Send + 'f,
    {
        let _scope = self.span.as_ref().cloned().map(SpanScope::new);
        self.span = Some(create_span(span_name));

        self.apply(fut)
    }
}

#[cfg(feature = "logging")]
impl TelemetryContext {
    /// [`TODO ROCK-13`]
    pub fn with_forked_log(&self) -> Self {
        Self {
            log: fork_log(),

            #[cfg(feature = "tracing")]
            span: self.span.clone(),

            #[cfg(all(feature = "tracing", feature = "testing"))]
            test_tracer: self.test_tracer.clone(),
        }
    }

    /// [`TODO ROCK-13`]
    pub fn slog_logger(&self) -> parking_lot::RwLockReadGuard<Logger> {
        self.log.read()
    }
}

/// [`TODO ROCK-13`]
pub fn init(service_info: ServiceInfo, settings: &TelemetrySettings) -> BootstrapResult<()> {
    #[cfg(feature = "logging")]
    self::log::init::init(service_info, &settings.logging)?;

    #[cfg(feature = "tracing")]
    self::tracing::init::init(service_info, &settings.tracing)?;

    Ok(())
}
