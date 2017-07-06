extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate time;

#[derive(Debug, PartialEq)]
pub enum Error {
    NegativeRTT,
    InvalidSync,
    OutOfLimitRTT,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
pub struct TimeSync {
    pub t1: u64,
    pub t2: u64,
    pub t3: u64,
    pub round: u32,
}

impl TimeSync {
    fn get_rtt(&self) -> Result<u64, Error> {
        if self.t1 == 0
        || self.t2 == 0
        || self.t3 == 0
        || self.t3 < self.t1 {
            return Err(Error::InvalidSync)
        }

        Ok(self.t3 - self.t1)
    }

    pub fn get_offset(&self) -> Result<i64, Error> {
        self.get_rtt().map(|rtt| {
            (self.t3 as i64 - self.t1 as i64 - rtt as i64 / 2) as i64
        })
    }
}

#[derive(Debug)]
pub struct TimeSyncer {
    pub messages : Vec<TimeSync>,
    pub size : u64,
    pub id : u32,
    pub bound : u32,
    pub factor : f32,
    pub round : u32,
    pub duration : u64,
    pub offset : i64,
    pub round_time : u64,
}

impl TimeSyncer {
    pub fn new(id : u32, size : u64, bound : u32, factor : f32, duration : u64) -> Self {
        TimeSyncer {
            messages : Vec::new(),
            size : size,
            id : id,
            bound : bound,
            factor : factor,
            round : 0,
            duration : duration,
            offset : 0,
            round_time : 0,
        }
    }
    pub fn time_now_ms(&self) -> u64 {
        let now = time::now().to_timespec();
        ((now.sec * 1000 as i64 + now.nsec as i64 / 1000000 as i64) + self.offset * self.factor as i64) as u64
    }

    pub fn next_round(&mut self) {
        let mut offset : i64 = 0;
        let mut count = 0;
        for s in self.messages.drain(..) {
            let s  : TimeSync = s;
            let ret = s.get_offset();
            if ret.is_ok() {
                let o = ret.unwrap();
                if o.abs() < self.bound as i64 {
                    count = count + 1;
                    offset = offset + o;
                }
            }
        }
        if count != 0 {
            self.offset = offset / count;
            info!("summary {} msg, offset chang to {:?}", count, self.offset);
        }
        self.round = self.round + 1;
        self.round_time = self.time_now_ms();
    }

    pub fn add_message(&mut self, mut msg : TimeSync) -> Result<(), Error> {
        msg.t3 = self.time_now_ms();
        if msg.round != self.round
        || msg.t1 != self.round_time {
            warn!("InvalidSync {:?}", msg);
            return Err(Error::InvalidSync)
        }
        info!("add sync msg {:?}", msg);
        self.messages.push(msg);
        Ok(())
    }
}



