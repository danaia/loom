use serde::{Deserialize, Serialize};

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);
    };
}

typed_id!(ValueId);
typed_id!(StreamId);
typed_id!(KernelId);
typed_id!(SlotId);
typed_id!(PassId);
typed_id!(ViewId);
typed_id!(ScheduleId);
typed_id!(ContractId);
typed_id!(ScenarioId);
typed_id!(BenchmarkId);
typed_id!(CapabilityId);
