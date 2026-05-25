use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;

const INF: u32 = 16;
static PRINT_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkConfig {
    routers: Vec<RouterConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouterConfig {
    ip: String,
    neighbors: Vec<NeighborConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NeighborConfig {
    ip: String,
    metric: u32,
}

#[derive(Debug, Clone)]
struct Route {
    destination: String,
    next_hop: String,
    metric: u32,
}

#[derive(Debug, Clone)]
struct RoutingUpdate {
    from: String,
    routes: Vec<Route>,
}

#[derive(Clone)]
struct Neighbor {
    ip: String,
    metric: u32,
    sender: mpsc::Sender<RoutingUpdate>,
}

struct Router {
    ip: String,
    neighbors: Vec<Neighbor>,
    table: HashMap<String, Route>,
    receiver: mpsc::Receiver<RoutingUpdate>,
}

#[tokio::main]
async fn main() {
    let configs = load_from_json("network.json");
    let mut senders = HashMap::new();
    let mut receivers = HashMap::new();

    for router in &configs {
        let (tx, rx) = mpsc::channel::<RoutingUpdate>(32);
        senders.insert(router.ip.clone(), tx);
        receivers.insert(router.ip.clone(), rx);
    }

    let mut routers = Vec::new();
    for router in configs {
        let receiver = receivers.remove(&router.ip).unwrap();
        let mut neighbors = Vec::new();

        for neighbor in router.neighbors {
            let sender = senders.get(&neighbor.ip).map(|tx| tx.clone()).unwrap();
            neighbors.push(Neighbor {
                ip: neighbor.ip,
                metric: neighbor.metric,
                sender,
            });
        }

        let mut table = HashMap::new();
        for neighbor in &neighbors {
            table.insert(
                neighbor.ip.clone(),
                Route {
                    destination: neighbor.ip.clone(),
                    next_hop: neighbor.ip.clone(),
                    metric: neighbor.metric,
                },
            );
        }

        routers.push(Router {
            ip: router.ip,
            neighbors,
            table,
            receiver,
        });
    }

    for router in routers {
        tokio::spawn(async move {
            router_task(router).await;
        });
    }

    sleep(Duration::from_secs(20)).await;
}

async fn router_task(mut router: Router) {
    println!("Router {} started", router.ip);

    loop {
        tokio::select! {
            Some(update) = router.receiver.recv() => {
                let neighbor_metric = router
                    .neighbors
                    .iter()
                    .find(|n| n.ip == update.from)
                    .map(|n| n.metric)
                    .unwrap_or(INF);

                let mut changed = false;
                for route in update.routes {
                    let new_metric = (route.metric + neighbor_metric).min(INF);
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
                        let new_route = Route {
                            destination: route.destination.clone(),
                            next_hop: update.from.clone(),
                            metric: new_metric,
                        };

                        let old_route = router.table.insert(
                            route.destination.clone(),
                            new_route.clone()
                        ).unwrap_or(Route {
                            destination: "-".to_string(),
                            next_hop: "-".to_string(),
                            metric: INF,
                        });

                        print_changes(
                            &router.ip,
                            &old_route,
                            &new_route
                        ).await;

                        changed = true;
                    }
                }

                if changed {
                    print_table(&router.ip, &router.table).await;
                }
            }
            _ = sleep(Duration::from_secs(1)) => {
                let routes = router.table.values().cloned().collect::<Vec<_>>();

                for neighbor in &router.neighbors {
                    let update = RoutingUpdate {
                        from: router.ip.clone(),
                        routes: routes.clone(),
                    };
                    let _ = neighbor.sender.send(update).await;
                }
            }
        }
    }
}

async fn print_changes(router_ip: &str, old_route: &Route, new_route: &Route) {
    let mg = PRINT_MUTEX.lock().await;
    println!("\nChanges in router {} table:", router_ip);
    println!(
        "{:<18} {:<18} {:<18} {:<6}",
        "[Source IP]", "[Destination IP]", "[Next Hop]", "[Metric]"
    );
    println!(
        "{:<18} {:<18} {:<18} {:<6}",
        router_ip, old_route.destination, old_route.next_hop, old_route.metric
    );
    println!(
        "{:<18} {:<18} {:<18} {:<6}",
        router_ip, new_route.destination, new_route.next_hop, new_route.metric
    );
    drop(mg);
}

async fn print_table(router_ip: &str, table: &HashMap<String, Route>) {
    let mg = PRINT_MUTEX.lock().await;
    println!("\nRouter {} table:", router_ip);
    println!(
        "{:<18} {:<18} {:<18} {:<6}",
        "[Source IP]", "[Destination IP]", "[Next Hop]", "[Metric]"
    );

    let mut routes = table.values().collect::<Vec<_>>();
    routes.sort_by(|a, b| a.destination.cmp(&b.destination));

    for route in routes {
        println!(
            "{:<18} {:<18} {:<18} {:<6}",
            router_ip, route.destination, route.next_hop, route.metric
        );
    }
    drop(mg);
}

fn load_from_json(path: &str) -> Vec<RouterConfig> {
    let data = fs::read_to_string(path).expect("Failed to read JSON");
    let config: NetworkConfig = serde_json::from_str(&data).expect("Invalid JSON");
    config.routers
}
