use chrono::{DateTime, Local, NaiveDate, Utc};
use clap::Clap;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::error::ClientError;

/*
 * Client has:
 *  - Name
 *  - Address
 *  - BillingRate
 */

/* Billing Rate has:
 * - A unit of billing
 * - An ammount to bill
 * - A currency for that ammount
 * - An effective date
 */

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum BillingUnit {
    Month,
    Week,
    Day,
    Hour,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Currency {
    CAD,
    USD,
    EUR,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let symbol = match self {
            Currency::CAD => "CAD $",
            Currency::USD => "USD $",
            Currency::EUR => "€",
        };

        write!(f, "{}", symbol)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BillingRate {
    amount: f32,
    currency: Currency,
    per: BillingUnit,
}

impl fmt::Display for BillingRate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{:.2}/{:?}", self.currency, self.amount, self.per)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Client {
    name: String,
    address: String,
    rates: BTreeMap<NaiveDate, BillingRate>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum ClientUpdate {
    Address(String),
    Rate(NaiveDate, BillingRate),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum ClientChange {
    Added { name: String, address: String },
    Updated(ClientUpdate),
    Removed,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct ClientEvent {
    key: String,
    timestamp: DateTime<Utc>,
    change: ClientChange,
}

fn load() -> Result<BTreeMap<String, Client>, ClientError> {
    let file = fs::File::open(Path::new("clients"))?;
    let reader = BufReader::new(file);
    let events: Vec<ClientEvent> = serde_lexpr::from_reader(reader)?;
    let client_map = from_events(events)?;
    Ok(client_map)
}

fn client(key: &str) -> Result<Client, ClientError> {
    let clients = load()?;
    match clients.get(key) {
        Some(client) => Ok(client.clone()),
        None => Err(ClientError::NotFound {key: key.to_string()})
    }
}

fn from_events(
    events: Vec<ClientEvent>,
) -> Result<BTreeMap<String, Client>, ClientError> {
    let mut clients = BTreeMap::new();
    for event in events.iter() {
        match &event.change {
            ClientChange::Added { name, address } => {
                clients.insert(event.key.clone(), Client::new(name, address));
            }
            ClientChange::Updated(update) => {
                clients
                    .get_mut(&event.key)
                    .map(|client| client.update(update));
            }
            ClientChange::Removed => {
                clients.remove(&event.key);
            }
        };
    }
    Ok(clients)
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} @ {}", self.name, self.address)
    }
}

#[derive(Clap)]
pub enum Command {
    List,
    Rate{key: String},
}

pub fn run_cmd(cmd: Command) -> Result<(), ClientError> {
    match cmd {
        Command::List => {
            let clients = load()?;
            for (key, client) in clients.iter() {
                println!("{}: {}", key, client);
            }
        }
        Command::Rate{key} =>  {
            let client = client(&key)?;
            if let Some(current) = client.current_rate() {
                println!("Current Rate: {}\n", current);
            } else {
                println!("No current rate for client: {}", client.name);
            }

            println!("Historical Rates:\n");
            for (effective, rate) in client.rates.iter() {
                println!("{} effective {}", rate, effective);
            }
        }
    }
    Ok(())
}

impl Client {
    fn new(name: &str, address: &str) -> Self {
        Self {
            name: name.to_string(),
            address: address.to_string(),
            rates: BTreeMap::new(),
        }
    }

    fn update(&mut self, update: &ClientUpdate) {
        match update {
            ClientUpdate::Address(addr) => self.address = addr.clone(),
            ClientUpdate::Rate(effective, rate) => {
                self.rates.insert(effective.clone(), rate.clone());
            }
        };
    }

    fn rate_as_of(&self, date: NaiveDate) -> Option<&BillingRate> {
        self.rates.range(..date).next_back().map(|(_, rate)| rate)
    }

    fn current_rate(&self) -> Option<&BillingRate> {
        let today = Local::today().naive_local();
        self.rate_as_of(today)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use const_format::formatcp;
    use serde_lexpr::{from_str, to_string, Error};
    use chrono::TimeZone;

    fn billing_rate() -> BillingRate {
        BillingRate {
            amount: 1000.0,
            currency: Currency::USD,
            per: BillingUnit::Month,
        }
    }

    const RATE_RAW: &str = "(amount . 1000.0) \
        (currency . USD) \
        (per . Month)";

    const CLIENT_RAW: &str = "(name . \"Innotech\") \
         (address . \"Some Place\") \
         (rates)";

    const CLIENT_STR: &str = formatcp!("({})", CLIENT_RAW);

    #[test]
    fn deserilize() {
        let client: Client = from_str(CLIENT_STR).unwrap();
        assert_eq!(client.name, "Innotech");
        assert_eq!(client.address, "Some Place");
    }

    #[test]
    fn serialize() -> Result<(), Error> {
        let client = Client::new("Innotech", "Some Place");
        let sexpr = to_string(&client)?;
        assert_eq!(sexpr, CLIENT_STR);
        Ok(())
    }

    const CLIENT_ADD_STR: &str = formatcp!(
        "((key . \"innotech\") \
          (timestamp . \"2021-04-15T10:30:00Z\") \
          (change Added (name . \"Innotech\") (address . \"Some Place\")))",
    );

    #[test]
    fn serialize_event() -> Result<(), Error> {
        let change = ClientChange::Added {
            name: "Innotech".to_string(),
            address: "Some Place".to_string(),
        };
        let event = ClientEvent {
            key: "innotech".to_string(),
            change: change,
            timestamp: Utc.ymd(2021, 04, 15).and_hms(10, 30, 0),
        };
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, CLIENT_ADD_STR);
        Ok(())
    }

    const RATE_UPDATE_STR: &str = formatcp!(
        "((key . \"innotech\") \
          (timestamp . \"2021-04-16T09:30:00Z\") \
          (change Updated Rate \"2021-04-15\" ({})))",
        RATE_RAW
    );

    #[test]
    fn serialize_update() -> Result<(), Error> {
        let update = ClientUpdate::Rate(
            NaiveDate::from_ymd(2021, 04, 15),
            billing_rate(),
        );
        let change = ClientChange::Updated(update);
        let event = ClientEvent {
            key: "innotech".to_string(),
            change: change,
            timestamp: Utc.ymd(2021, 04, 16).and_hms(9, 30, 0),
        };
        let sexpr = to_string(&event)?;
        assert_eq!(sexpr, RATE_UPDATE_STR);
        Ok(())
    }

    const EVENTS_STR: &str =
        formatcp!("({}\n{})", CLIENT_ADD_STR, RATE_UPDATE_STR);

    #[test]
    fn client_from_events() -> Result<(), Error> {
        let events: Vec<ClientEvent> = from_str(EVENTS_STR)?;
        let client_map = from_events(events).unwrap();

        let client = client_map.get("innotech").unwrap();
        let query_date = NaiveDate::from_ymd(2021, 04, 17);

        assert_eq!(&client.address, "Some Place");
        assert_eq!(client.rate_as_of(query_date), Some(&billing_rate()));
        Ok(())
    }

    #[test]
    fn list() -> Result<(), ClientError> {
        run_cmd(Command::List)?;
        Ok(())
    }
}

/*
#[cfg(test)]
mod proptests {
    use super::*;
    use chrono_utilities::naive::DateTransitions;
    use proptest::prelude::*;

    fn arb_currency() -> impl Strategy<Value = Currency> {
        prop_oneof![
            Just(Currency::CAD),
            Just(Currency::USD),
            Just(Currency::EUR),
        ]
    }

    fn arb_billing_unit() -> impl Strategy<Value = BillingUnit> {
        prop_oneof![
            Just(BillingUnit::Month),
            Just(BillingUnit::Week),
            Just(BillingUnit::Day),
            Just(BillingUnit::Hour),
        ]
    }

    prop_compose! {
        fn arb_date() (y in 1..10000i32, m in 1..13u32)
            (d in 1..NaiveDate::from_ymd(y, m, 1).last_day_of_month() + 1,
             y in Just(y),
             m in Just(m))
            -> NaiveDate {
            NaiveDate::from_ymd(y,m,d)
        }

    }

    prop_compose! {
        fn arb_billing_rate()
            (amount in any::<f32>().prop_map(f32::abs),
             currency in arb_currency(),
             per in arb_billing_unit()) -> BillingRate {

            BillingRate { amount, currency, per}
        }
    }

    prop_compose! {
        fn arb_client()
            (name in ".*",
             address in ".*") -> Client {
            Client::new(&name, &address)
        }
    }

    prop_compose! {
        fn arb_clients(max_size: usize)
            (vec in prop::collection::vec(arb_client(), 1..max_size))
        -> Vec<Client> {
            vec
        }
    }

    /*
    prop_compose! {
        /*
         * Start with added event
         * random number of events to follow
         * generate a vector of durations as time between events
         * generate a start date
         * map start date + durations to a sequence of increasing timestamps
         * generate a remove boolean
         * generate a sequence of updates len = durations - add [-remove]
         * map add + updates + removal to a sequence of events
         */
        fn client_events()
            (client in arb_client,
             key in "[a-z_]+",
             num_events in 0..100)
            (cli



    }
    */

    proptest! {
        #[test]
        fn test_client(clients in arb_clients(5)) {
            println!("{:?}", clients);
        }
    }
}*/
