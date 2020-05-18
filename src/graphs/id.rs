#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GraphIdArg {
    pub id: GraphId,
    pub arg: Option<u64>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GraphId {
    pub node: u64,
    pub pass: u64,
    pub repeat: u64,
}

impl GraphId {
    pub fn new_input() -> Self {
        Self {
            node: 0,
            pass: 0,
            repeat: 0,
        }
    }

    pub fn new_first() -> Self {
        Self {
            node: 1,
            pass: 0,
            repeat: 0,
        }
    }
}

impl GraphId {
    pub fn validate(&self, last: &Self) -> bool {
        if self.node == last.node {
            if self.pass == last.pass {
                self.repeat == last.repeat + 1
            } else {
                self.pass == last.pass + 1
            }
        } else {
            self.node == last.node + 1
        }
    }

    pub fn is_input(&self) -> bool {
        self == &Self::new_input()
    }

    pub fn is_first(&self) -> bool {
        self == &Self::new_first()
    }
}
