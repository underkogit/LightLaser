use crate::core::layer::Project;

pub struct History {
    undo_stack: Vec<Project>,
    redo_stack: Vec<Project>,
    limit: usize,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            limit,
        }
    }

    pub fn push(&mut self, project: &Project) {
        self.push_owned(project.clone());
    }

    pub fn push_owned(&mut self, project: Project) {
        if self.undo_stack.len() >= self.limit {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(project);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, current: &Project) -> Option<Project> {
        if let Some(prev) = self.undo_stack.pop() {
            if self.redo_stack.len() >= self.limit {
                self.redo_stack.remove(0);
            }
            self.redo_stack.push(current.clone());
            Some(prev)
        } else {
            None
        }
    }

    pub fn redo(&mut self, current: &Project) -> Option<Project> {
        if let Some(next) = self.redo_stack.pop() {
            if self.undo_stack.len() >= self.limit {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(current.clone());
            Some(next)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new(100)
    }
}
