use crate::{cargo::BuiltApplication, watch::ChangeImpact};

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub(crate) enum DevEvent {
    Start,
    Changed(ChangeImpact),
    BuildSucceeded(BuiltApplication),
    BuildFailed,
    ApplicationExited,
    Shutdown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DevAction {
    Build,
    Start(BuiltApplication),
    Stop,
    Restart(BuiltApplication),
    Exit,
}

pub(crate) struct DevState {
    last_successful: Option<BuiltApplication>,
    application_running: bool,
    build_running: bool,
    pending: ChangeImpact,
    shutting_down: bool,
}

impl DevState {
    pub(crate) fn new() -> Self {
        Self {
            last_successful: None,
            application_running: false,
            build_running: false,
            pending: ChangeImpact::None,
            shutting_down: false,
        }
    }

    pub(crate) fn transition(&mut self, event: DevEvent) -> Vec<DevAction> {
        if self.shutting_down {
            return Vec::new();
        }

        match event {
            DevEvent::Start => self.start_build(),
            DevEvent::Changed(impact) => self.handle_change(impact),
            DevEvent::BuildSucceeded(built) => self.handle_build_succeeded(built),
            DevEvent::BuildFailed => self.handle_build_failed(),
            DevEvent::ApplicationExited => {
                self.application_running = false;
                Vec::new()
            }
            DevEvent::Shutdown => self.shutdown(),
        }
    }

    fn start_build(&mut self) -> Vec<DevAction> {
        if self.build_running {
            return Vec::new();
        }

        self.build_running = true;
        vec![DevAction::Build]
    }

    fn handle_change(&mut self, impact: ChangeImpact) -> Vec<DevAction> {
        if impact == ChangeImpact::None {
            return Vec::new();
        }
        if self.build_running {
            self.pending = ChangeImpact::merge([self.pending, impact]);
            return Vec::new();
        }

        match impact {
            ChangeImpact::None => Vec::new(),
            ChangeImpact::Restart => self.restart_last_successful(),
            ChangeImpact::Rebuild => self.start_build(),
        }
    }

    fn restart_last_successful(&mut self) -> Vec<DevAction> {
        let Some(built) = self.last_successful.clone() else {
            return Vec::new();
        };

        if self.application_running {
            vec![DevAction::Restart(built)]
        } else {
            self.application_running = true;
            vec![DevAction::Start(built)]
        }
    }

    fn handle_build_succeeded(&mut self, built: BuiltApplication) -> Vec<DevAction> {
        self.build_running = false;
        let pending = std::mem::replace(&mut self.pending, ChangeImpact::None);
        self.last_successful = Some(built.clone());

        let mut actions = if self.application_running {
            vec![DevAction::Stop, DevAction::Start(built)]
        } else {
            vec![DevAction::Start(built)]
        };
        self.application_running = true;

        if pending == ChangeImpact::Rebuild {
            actions.extend(self.start_build());
        }
        actions
    }

    fn handle_build_failed(&mut self) -> Vec<DevAction> {
        self.build_running = false;
        let pending = std::mem::replace(&mut self.pending, ChangeImpact::None);

        if pending == ChangeImpact::Rebuild {
            self.start_build()
        } else {
            Vec::new()
        }
    }

    fn shutdown(&mut self) -> Vec<DevAction> {
        self.shutting_down = true;
        self.build_running = false;
        self.pending = ChangeImpact::None;

        let mut actions = Vec::new();
        if self.application_running {
            self.application_running = false;
            actions.push(DevAction::Stop);
        }
        actions.push(DevAction::Exit);
        actions
    }
}

#[cfg(test)]
mod tests {
    use crate::{cargo::BuiltApplication, watch::ChangeImpact};

    use super::{DevAction, DevEvent, DevState};

    #[test]
    fn starts_builds_restarts_and_rebuilds_an_application() {
        let mut state = DevState::new();
        let v1 = binary("v1");

        assert_eq!(state.transition(DevEvent::Start), [DevAction::Build]);
        assert_eq!(
            state.transition(DevEvent::BuildSucceeded(v1.clone())),
            [DevAction::Start(v1.clone())],
        );
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Restart)),
            [DevAction::Restart(v1)],
        );
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
    }

    #[test]
    fn initial_build_failure_keeps_watching_without_an_application() {
        let mut state = DevState::new();

        assert_eq!(state.transition(DevEvent::Start), [DevAction::Build]);
        assert!(state.transition(DevEvent::BuildFailed).is_empty());
        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Restart))
                .is_empty()
        );
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
    }

    #[test]
    fn configuration_change_without_a_successful_binary_is_a_noop() {
        let mut state = DevState::new();

        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Restart))
                .is_empty()
        );
    }

    #[test]
    fn failed_rebuild_keeps_the_active_application_available_for_restart() {
        let mut state = running_state("v1");
        let v1 = binary("v1");

        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
        assert!(state.transition(DevEvent::BuildFailed).is_empty());
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Restart)),
            [DevAction::Restart(v1)],
        );
    }

    #[test]
    fn successful_rebuild_stops_the_old_application_before_starting_the_new_one() {
        let mut state = running_state("v1");
        let v2 = binary("v2");

        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
        assert_eq!(
            state.transition(DevEvent::BuildSucceeded(v2.clone())),
            [DevAction::Stop, DevAction::Start(v2)],
        );
    }

    #[test]
    fn restart_during_a_build_is_consumed_by_the_replacement_start() {
        let mut state = running_state("v1");
        let v2 = binary("v2");

        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Restart))
                .is_empty()
        );
        assert_eq!(
            state.transition(DevEvent::BuildSucceeded(v2.clone())),
            [DevAction::Stop, DevAction::Start(v2)],
        );
    }

    #[test]
    fn restart_queued_during_a_failed_build_does_not_trigger_another_build() {
        let mut state = running_state("v1");
        let v1 = binary("v1");

        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Restart))
                .is_empty()
        );
        assert!(state.transition(DevEvent::BuildFailed).is_empty());
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Restart)),
            [DevAction::Restart(v1)],
        );
    }

    #[test]
    fn rebuild_events_during_a_build_coalesce_to_one_follow_up_build() {
        let mut state = running_state("v1");
        let v2 = binary("v2");

        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Rebuild))
                .is_empty()
        );
        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Rebuild))
                .is_empty()
        );
        assert_eq!(
            state.transition(DevEvent::BuildSucceeded(v2.clone())),
            [DevAction::Stop, DevAction::Start(v2), DevAction::Build],
        );
        assert!(state.transition(DevEvent::BuildFailed).is_empty());
    }

    #[test]
    fn merged_restart_events_result_in_one_restart() {
        let mut state = running_state("v1");
        let v1 = binary("v1");
        let change = ChangeImpact::merge([ChangeImpact::Restart, ChangeImpact::Restart]);

        assert_eq!(
            state.transition(DevEvent::Changed(change)),
            [DevAction::Restart(v1)],
        );
    }

    #[test]
    fn unexpected_exit_retains_the_binary_for_later_relevant_changes() {
        let mut state = running_state("v1");
        let v1 = binary("v1");

        assert!(state.transition(DevEvent::ApplicationExited).is_empty());
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Restart)),
            [DevAction::Start(v1)],
        );
        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
    }

    #[test]
    fn shutdown_stops_an_active_application_and_exits_even_while_building() {
        let mut state = running_state("v1");

        assert_eq!(
            state.transition(DevEvent::Changed(ChangeImpact::Rebuild)),
            [DevAction::Build],
        );
        assert_eq!(
            state.transition(DevEvent::Shutdown),
            [DevAction::Stop, DevAction::Exit],
        );
        assert!(
            state
                .transition(DevEvent::BuildSucceeded(binary("v2")))
                .is_empty()
        );
        assert!(
            state
                .transition(DevEvent::Changed(ChangeImpact::Rebuild))
                .is_empty()
        );
    }

    fn running_state(version: &str) -> DevState {
        let mut state = DevState::new();
        state.transition(DevEvent::Start);
        state.transition(DevEvent::BuildSucceeded(binary(version)));
        state
    }

    fn binary(version: &str) -> BuiltApplication {
        BuiltApplication::test_identity(version)
    }
}
