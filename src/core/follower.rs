use crate::core::{RaftCore, State};
use crate::msg::Message;
use crate::{Event, RaftType};
use crossbeam_channel::RecvTimeoutError;

pub struct Follower<'a, T: RaftType> {
    core: &'a mut RaftCore<T>,
}

impl<'a, T: RaftType> Follower<'a, T> {
    #[inline]
    pub fn new(core: &'a mut RaftCore<T>) -> Self {
        Self { core }
    }

    pub fn run(mut self) {
        // Use set_prev_state to ensure prev_state can be set at most once.
        let mut set_prev_state = Some(true);

        assert!(self.core.is_state(State::Follower));
        self.core.next_election_timeout = None;
        let _result = self.core.spawn_event_handling_task(Event::TransitToFollower {
            term: self.core.hard_state.current_term,
            prev_state: self.core.prev_state(),
        });
        self.core.report_metrics();

        info!("[Node({})] start the follower loop", self.core.node_id);

        loop {
            if !self.core.is_state(State::Follower) {
                return;
            }

            let election_timeout = self.core.next_election_timeout();

            match self.core.msg_rx.recv_deadline(election_timeout) {
                Ok(msg) => match msg {
                    Message::Heartbeat { req, tx } => {
                        trace!("[Node({})] received heartbeat: {:?}", self.core.node_id, req);

                        let result = self.core.handle_heartbeat(req, set_prev_state.as_mut());
                        if let Err(ref e) = result {
                            debug!(
                                "[Node({})] failed to handle heartbeat request: {}",
                                self.core.node_id, e
                            );
                        }
                        let _ = tx.send(result);
                    }
                    Message::HeartbeatResponse(_) => {
                        // ignore heartbeat response
                    }
                    Message::VoteRequest { req, tx } => {
                        let result = self.core.handle_vote_request(req, set_prev_state.as_mut());
                        if let Err(ref e) = result {
                            debug!("[Node({})] failed to handle vote request: {}", self.core.node_id, e);
                        }
                        let _ = tx.send(result);
                    }
                    Message::VoteResponse { .. } => {
                        // ignore vote response
                    }
                    Message::Initialize { tx, .. } => {
                        self.core.reject_init_with_members(tx);
                    }
                    Message::UpdateOptions { options, tx } => {
                        info!("[Node({})] raft update options: {:?}", self.core.node_id, options);
                        self.core.update_options(options);
                        let _ = tx.send(Ok(()));
                    }
                    Message::Shutdown => {
                        info!("[Node({})] raft received shutdown message", self.core.node_id);
                        self.core.set_state(State::Shutdown, set_prev_state.as_mut());
                    }
                    Message::EventHandlingResult { event, error, term } => {
                        if let Some(e) = error {
                            error!(
                                "[Node({})] raft failed to handle event ({:?}) in term {}: {} ",
                                self.core.node_id, event, term, e
                            );
                        } else {
                        }
                    }
                },
                Err(e) => match e {
                    RecvTimeoutError::Timeout => {
                        self.core.set_state(State::PreCandidate, set_prev_state.as_mut());
                        self.core.current_leader = None;
                        info!(
                            "[Node({})] an election timeout is hit, need to transit to pre-candidate",
                            self.core.node_id
                        );
                    }
                    RecvTimeoutError::Disconnected => {
                        info!("[Node({})] the raft message channel is disconnected", self.core.node_id);
                        self.core.set_state(State::Shutdown, set_prev_state.as_mut());
                    }
                },
            }
        }
    }
}
