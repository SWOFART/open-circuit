use crate::{PinId, devices::Terminals};

/// A branch declaration shared by geometry resolution and result layout.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PinGroup {
    pub id: &'static str,
    pub pins: [PinId; 2],
}

impl PinGroup {
    pub(crate) fn resolve(
        self,
        node_for_pin: impl Fn(PinId) -> Option<usize>,
    ) -> Option<Terminals> {
        if self.pins[0] == self.pins[1] {
            return None;
        }
        Some(Terminals {
            pin1: node_for_pin(self.pins[0])?,
            pin2: node_for_pin(self.pins[1])?,
        })
    }
}

pub(crate) const CONTACTOR_COIL: PinGroup = PinGroup {
    id: "coil",
    pins: [PinId::A1, PinId::A2],
};
pub(crate) const CONTACTOR_AUXILIARIES: [PinGroup; 2] = [
    PinGroup {
        id: "no",
        pins: [PinId::No1, PinId::No2],
    },
    PinGroup {
        id: "nc",
        pins: [PinId::Nc1, PinId::Nc2],
    },
];

pub(crate) const POWER_POLES: [PinGroup; 3] = [
    PinGroup {
        id: "pole1",
        pins: [PinId::L1, PinId::T1],
    },
    PinGroup {
        id: "pole2",
        pins: [PinId::L2, PinId::T2],
    },
    PinGroup {
        id: "pole3",
        pins: [PinId::L3, PinId::T3],
    },
];

pub(crate) const INDUCTION_WINDINGS: [PinGroup; 3] = [
    PinGroup {
        id: "u",
        pins: [PinId::U1, PinId::U2],
    },
    PinGroup {
        id: "v",
        pins: [PinId::V1, PinId::V2],
    },
    PinGroup {
        id: "w",
        pins: [PinId::W1, PinId::W2],
    },
];

pub(crate) const TRANSFORMER_WINDINGS: [PinGroup; 6] = [
    PinGroup {
        id: "primary1",
        pins: [PinId::P1a, PinId::P1b],
    },
    PinGroup {
        id: "primary2",
        pins: [PinId::P2a, PinId::P2b],
    },
    PinGroup {
        id: "primary3",
        pins: [PinId::P3a, PinId::P3b],
    },
    PinGroup {
        id: "secondary1",
        pins: [PinId::S1a, PinId::S1b],
    },
    PinGroup {
        id: "secondary2",
        pins: [PinId::S2a, PinId::S2b],
    },
    PinGroup {
        id: "secondary3",
        pins: [PinId::S3a, PinId::S3b],
    },
];
pub(crate) const STAR_LOAD_WINDINGS: [PinGroup; 3] = [
    PinGroup {
        id: "l1",
        pins: [PinId::L1, PinId::N],
    },
    PinGroup {
        id: "l2",
        pins: [PinId::L2, PinId::N],
    },
    PinGroup {
        id: "l3",
        pins: [PinId::L3, PinId::N],
    },
];
pub(crate) const DELTA_LOAD_WINDINGS: [PinGroup; 3] = [
    PinGroup {
        id: "l12",
        pins: [PinId::L1, PinId::L2],
    },
    PinGroup {
        id: "l23",
        pins: [PinId::L2, PinId::L3],
    },
    PinGroup {
        id: "l31",
        pins: [PinId::L3, PinId::L1],
    },
];

/// The poles of the residual-current devices (docs/contracts/stage-13-protection-and-safety.md
/// section 4.1): the third and fourth pole of the four-pole device carry L3 and the neutral.
pub(crate) const RCD_TWO_POLES: [PinGroup; 2] = [
    PinGroup {
        id: "pole1",
        pins: [PinId::L1, PinId::T1],
    },
    PinGroup {
        id: "pole2",
        pins: [PinId::L2, PinId::T2],
    },
];
pub(crate) const RCD_FOUR_POLES: [PinGroup; 4] = [
    PinGroup {
        id: "pole1",
        pins: [PinId::L1, PinId::T1],
    },
    PinGroup {
        id: "pole2",
        pins: [PinId::L2, PinId::T2],
    },
    PinGroup {
        id: "pole3",
        pins: [PinId::L3, PinId::T3],
    },
    PinGroup {
        id: "pole4",
        pins: [PinId::L4, PinId::T4],
    },
];
