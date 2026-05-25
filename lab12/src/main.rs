use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

const INF: u32 = 16;

#[derive(Debug, Clone)]
struct Route {
    destination: String,
    next_hop: String,
    metric: u32,
}

#[derive(Debug, Clone)]
struct Router {
    ip: String,
    neighbors: Vec<Neighbor>,
    table: HashMap<String, Route>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkConfig {
    routers: Vec<RouterConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RouterConfig {
    ip: String,
    neighbors: Vec<Neighbor>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Neighbor {
    ip: String,
    metric: u32,
}

fn main() {
    let routers = load_from_json("network.json");
    let mut network: HashMap<String, Router> = HashMap::new();

    for r in routers {
        let mut table = HashMap::new();

        for neighbor in &r.neighbors {
            table.insert(
                neighbor.ip.clone(),
                Route {
                    destination: neighbor.ip.clone(),
                    next_hop: neighbor.ip.clone(),
                    metric: neighbor.metric,
                },
            );
        }

        network.insert(
            r.ip.clone(),
            Router {
                ip: r.ip,
                neighbors: r.neighbors,
                table,
            },
        );
    }

    simulate_rip(&mut network);

    print_tables(&network);
}

fn simulate_rip(network: &mut HashMap<String, Router>) {
    let mut changed = true;
    let mut iteration = 0;

    while changed {
        changed = false;
        iteration += 1;

        println!("=== RIP iteration {} ===", iteration);

        let snapshot = network.clone();

        for router in network.values_mut() {
            for neighbor in &router.neighbors {
                let neighbor_router = snapshot.get(&neighbor.ip).unwrap();

                for route in neighbor_router.table.values() {
                    let new_metric = (route.metric + neighbor.metric).min(INF);

                    let should_update = match router.table.get(&route.destination) {
                        Some(existing) => new_metric < existing.metric,
                        None => {
                            if router.ip == route.destination {
                                false
                            } else {
                                true
                            }
                        }
                    };

                    if should_update {
                        router.table.insert(
                            route.destination.clone(),
                            Route {
                                destination: route.destination.clone(),
                                next_hop: neighbor.ip.clone(),
                                metric: new_metric,
                            },
                        );

                        changed = true;
                    }
                }
            }
        }
    }
}

fn print_tables(network: &HashMap<String, Router>) {
    for router in network.values() {
        println!();
        println!("Final state of router {} table:", router.ip);

        println!(
            "{:<18} {:<18} {:<18} {:<6}",
            "[Source IP]", "[Destination IP]", "[Next Hop]", "[Metric]"
        );

        let mut routes: Vec<_> = router.table.values().collect();
        routes.sort_by(|a, b| a.destination.cmp(&b.destination));

        for route in routes {
            println!(
                "{:<18} {:<18} {:<18} {:<6}",
                router.ip, route.destination, route.next_hop, route.metric
            );
        }
    }
}

fn load_from_json(path: &str) -> Vec<Router> {
    let data = fs::read_to_string(path).expect("Failed to read JSON file");

    let config: NetworkConfig = serde_json::from_str(&data).expect("Invalid JSON");

    config
        .routers
        .into_iter()
        .map(|r| Router {
            ip: r.ip,
            neighbors: r.neighbors,
            table: HashMap::new(),
        })
        .collect()
}
