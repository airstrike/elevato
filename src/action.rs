//! What a TEA cell's `update` hands back to its parent: an optional
//! `Instruction` the parent must act on (state it owns — playback,
//! storage, navigation) and a [`Task`] the parent must return to the
//! iced runtime, suitably `.map`ped.

use iced::Task;

/// The result of a child cell's `update`.
#[must_use = "the task must be returned to iced and the instruction matched by the parent"]
pub struct Action<I, Message> {
    /// A demand on the parent, matched in place.
    pub instruction: Option<I>,
    /// Work for the iced runtime, `.map`ped into the parent's message.
    pub task: Task<Message>,
}

impl<I, Message> Action<I, Message> {
    /// Neither an instruction nor a task.
    pub fn none() -> Self {
        Self {
            instruction: None,
            task: Task::none(),
        }
    }

    /// Both an instruction and a task.
    pub fn new(instruction: I, task: Task<Message>) -> Self {
        Self {
            instruction: Some(instruction),
            task,
        }
    }

    /// An instruction for the parent, with no task.
    pub fn instruction(instruction: I) -> Self {
        Self {
            instruction: Some(instruction),
            task: Task::none(),
        }
    }

    /// A task for the runtime, with no instruction.
    pub fn task(task: Task<Message>) -> Self {
        Self {
            instruction: None,
            task,
        }
    }

    /// Maps the task's message into another type.
    pub fn map<N>(self, f: impl Fn(Message) -> N + Send + 'static) -> Action<I, N>
    where
        Message: Send + 'static,
        N: Send + 'static,
    {
        Action {
            instruction: self.instruction,
            task: self.task.map(f),
        }
    }

    /// Maps the instruction into another type — for re-bubbling a
    /// child's instruction through an intermediate layer unchanged.
    pub fn map_instruction<N>(self, f: impl Fn(I) -> N + Send + 'static) -> Action<N, Message>
    where
        I: Send + 'static,
        N: Send + 'static,
    {
        Action {
            instruction: self.instruction.map(f),
            task: self.task,
        }
    }

    /// Sets the instruction.
    pub fn with_instruction(mut self, instruction: I) -> Self {
        self.instruction = Some(instruction);
        self
    }

    /// Sets the task.
    pub fn with_task(mut self, task: Task<Message>) -> Self {
        self.task = task;
        self
    }
}

impl<I: std::fmt::Debug, Message> std::fmt::Debug for Action<I, Message> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            .field("instruction", &self.instruction)
            .finish_non_exhaustive()
    }
}
