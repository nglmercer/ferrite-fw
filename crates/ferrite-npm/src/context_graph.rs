//! Contextual occurrence graph followed by deterministic bisimulation folding.
use crate::{range_satisfied, Installer, LockedPackage, Lockfile, RegistryVersion};
use ferrite_core::{FerriteError, Result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone)]
struct Node {
    info: RegistryVersion,
    dependencies: BTreeMap<String, usize>,
    peers: BTreeMap<String, usize>,
}
struct Graph<'a> {
    installer: &'a Installer,
    previous: &'a Lockfile,
    project: Option<&'a crate::workspace::ProjectSources>,
    nodes: Vec<Node>,
}
impl Graph<'_> {
    fn allocate(&mut self, info: RegistryVersion) -> Result<usize> {
        if self.nodes.len() >= 20_000 {
            return Err(FerriteError::Npm(
                "dependency occurrence limit exceeded".into(),
            ));
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            info,
            dependencies: BTreeMap::new(),
            peers: BTreeMap::new(),
        });
        Ok(index)
    }
    fn expand<'a>(
        &'a mut self,
        index: usize,
        providers: BTreeMap<String, usize>,
        mut ancestors: Vec<usize>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if ancestors.len() >= 256 {
                return Err(FerriteError::Npm(
                    "dependency nesting limit exceeded".into(),
                ));
            }
            let info = self.nodes[index].info.clone();
            for (name, range) in &info.peer_dependencies {
                match providers.get(name) {
                    Some(provider)
                        if range_satisfied(&self.nodes[*provider].info.version, range) =>
                    {
                        self.nodes[index].peers.insert(name.clone(), *provider);
                    }
                    None if info
                        .peer_dependencies_meta
                        .get(name)
                        .is_some_and(|meta| meta.optional) =>
                    {}
                    _ => {
                        return Err(FerriteError::Npm(format!(
                            "{}@{} needs peer {name}@{range} compatible with its importer in its importer",
                            info.name, info.version
                        )))
                    }
                }
            }
            ancestors.push(index);
            let mut requests: BTreeMap<_, _> = info.dependencies.into_iter().collect();
            requests.extend(info.optional_dependencies);
            let mut children = BTreeMap::new();
            let mut fresh = Vec::new();
            for (name, range) in requests {
                let previous = self
                    .previous
                    .find(&info.name)
                    .filter(|package| package.version == info.version);
                let local = self
                    .project
                    .map(|project| {
                        project.select(
                            &name,
                            &range,
                            info.local
                                .as_ref()
                                .map_or(".", |source| source.path.as_str()),
                        )
                    })
                    .transpose()?
                    .flatten();
                let child = if let Some(local) = local {
                    local
                } else {
                    self.installer
                        .select(
                            &name,
                            &range,
                            previous
                                .and_then(|package| package.dependencies.get(&name))
                                .map(String::as_str),
                            previous
                                .and_then(|package| package.dependency_specifiers.get(&name))
                                .map(String::as_str),
                            self.previous,
                        )
                        .await?
                };
                let existing = ancestors.iter().rev().copied().find(|ancestor| {
                    let other = &self.nodes[*ancestor].info;
                    other.name == child.name
                        && other.version == child.version
                        && other.local.as_ref().map(|source| &source.path)
                            == child.local.as_ref().map(|source| &source.path)
                });
                let child = if let Some(existing) = existing {
                    existing
                } else {
                    let child = self.allocate(child)?;
                    fresh.push(child);
                    child
                };
                children.insert(name, child);
            }
            let mut child_providers = providers;
            child_providers.insert(info.name, index);
            child_providers.extend(children.clone());
            self.nodes[index].dependencies = children;
            for child in fresh {
                self.expand(child, child_providers.clone(), ancestors.clone())
                    .await?;
            }
            Ok(())
        })
    }
}
fn intrinsic(info: &RegistryVersion) -> String {
    let mut value = serde_json::to_value(info).expect("registry metadata serializes");
    if let Some(local) = &info.local {
        value["__ferrite_local"] = serde_json::to_value(local).expect("local metadata serializes");
    }
    value.sort_all_objects();
    serde_json::to_string(&value).expect("JSON value serializes")
}

fn colors(signatures: Vec<String>) -> Vec<usize> {
    let unique: BTreeSet<_> = signatures.iter().cloned().collect();
    let mapping: BTreeMap<_, _> = unique
        .into_iter()
        .enumerate()
        .map(|(index, value)| (value, index))
        .collect();
    signatures.iter().map(|value| mapping[value]).collect()
}
pub(crate) async fn resolve(
    installer: &Installer,
    roots: BTreeMap<String, RegistryVersion>,
    previous: &Lockfile,
) -> Result<(Vec<LockedPackage>, BTreeMap<String, String>)> {
    let (packages, mut importers) = resolve_importers(
        installer,
        BTreeMap::from([(".".into(), roots)]),
        previous,
        None,
    )
    .await?;
    Ok((packages, importers.remove(".").expect("root importer")))
}

pub(crate) async fn resolve_importers(
    installer: &Installer,
    roots: BTreeMap<String, BTreeMap<String, RegistryVersion>>,
    previous: &Lockfile,
    project: Option<&crate::workspace::ProjectSources>,
) -> Result<(
    Vec<LockedPackage>,
    BTreeMap<String, BTreeMap<String, String>>,
)> {
    let mut graph = Graph {
        installer,
        previous,
        project,
        nodes: Vec::new(),
    };
    let mut importers = BTreeMap::new();
    for (path, infos) in roots {
        let mut indices = BTreeMap::new();
        for (name, info) in infos {
            indices.insert(name, graph.allocate(info)?);
        }
        importers.insert(path, indices);
    }
    for roots in importers.values() {
        let mut providers = importers.get(".").cloned().unwrap_or_default();
        providers.extend(roots.clone());
        if let Some(index) = roots.get("\0workspace") {
            providers.insert(graph.nodes[*index].info.name.clone(), *index);
        }
        for index in roots.values().copied() {
            graph.expand(index, providers.clone(), Vec::new()).await?;
        }
    }
    let nodes = &graph.nodes;
    let metadata: Vec<_> = nodes.iter().map(|node| intrinsic(&node.info)).collect();
    let mut partition = colors(metadata.clone());
    loop {
        let signatures = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let dependencies: BTreeMap<_, _> = node
                    .dependencies
                    .iter()
                    .map(|(name, target)| (name, partition[*target]))
                    .collect();
                let peers: BTreeMap<_, _> = node
                    .peers
                    .iter()
                    .map(|(name, target)| (name, partition[*target]))
                    .collect();
                serde_json::to_string(&(partition[index], dependencies, peers)).unwrap()
            })
            .collect();
        let refined = colors(signatures);
        let stable = refined.iter().collect::<BTreeSet<_>>().len()
            == partition.iter().collect::<BTreeSet<_>>().len();
        partition = refined;
        if stable {
            break;
        }
    }
    let representatives: BTreeMap<_, _> = partition
        .iter()
        .copied()
        .enumerate()
        .map(|(index, class)| (class, index))
        .collect();
    let mut counts = BTreeMap::new();
    for index in representatives.values() {
        let info = &nodes[*index].info;
        *counts.entry((&info.name, &info.version)).or_insert(0) += 1;
    }
    let mut ids = BTreeMap::new();
    let mut contexts = BTreeMap::new();
    for (class, index) in &representatives {
        let node = &nodes[*index];
        let base = format!("{}@{}", node.info.name, node.info.version);
        if node.info.local.is_none()
            && node.peers.is_empty()
            && counts[&(&node.info.name, &node.info.version)] == 1
        {
            ids.insert(*class, base);
            continue;
        }
        // Rooted quotient serialization gives stable fingerprints even when peer
        // providers have their own peer cycles. No circular hash iteration.
        let mut pending = VecDeque::from([*class]);
        let mut local = BTreeMap::from([(*class, 0usize)]);
        let mut fingerprint = Vec::new();
        while let Some(current) = pending.pop_front() {
            let representative = representatives[&current];
            let node = &nodes[representative];
            let mut edge_maps = Vec::new();
            for edges in [&node.dependencies, &node.peers] {
                let mut mapped = BTreeMap::new();
                for (name, target) in edges {
                    let target = partition[*target];
                    let next = local.len();
                    let target_id = *local.entry(target).or_insert_with(|| {
                        pending.push_back(target);
                        next
                    });
                    mapped.insert(name, target_id);
                }
                edge_maps.push(mapped);
            }
            fingerprint.push((&metadata[representative], edge_maps));
        }
        let hash = ferrite_core::Hash::of_str(&serde_json::to_string(&fingerprint).unwrap()).0;
        ids.insert(*class, format!("{base}__context_{hash}"));
        contexts.insert(*class, hash);
    }
    let mut packages = Vec::new();
    for (class, index) in representatives {
        let node = &nodes[index];
        let info = &node.info;
        let mut requests: BTreeMap<_, _> = info.dependencies.clone().into_iter().collect();
        requests.extend(info.optional_dependencies.clone());
        let peers: BTreeMap<_, _> = node
            .peers
            .iter()
            .map(|(name, target)| (name.clone(), ids[&partition[*target]].clone()))
            .collect();
        let mut dependencies: BTreeMap<_, _> = node
            .dependencies
            .iter()
            .map(|(name, target)| (name.clone(), ids[&partition[*target]].clone()))
            .collect();
        for (name, target) in &peers {
            dependencies
                .entry(name.clone())
                .or_insert_with(|| target.clone());
        }
        packages.push(LockedPackage {
            name: info.name.clone(),
            version: info.version.clone(),
            source: info
                .local
                .as_ref()
                .map_or("npm", |source| source.kind.as_str())
                .into(),
            path: info.local.as_ref().map(|source| source.path.clone()),
            integrity: Some(if let Some(local) = &info.local {
                local.manifest_hash.clone()
            } else {
                crate::package::distribution_integrity(&info.dist)?
            }),
            tarball: info.local.is_none().then(|| info.dist.tarball.clone()),
            context: contexts.get(&class).cloned(),
            dependencies,
            dependency_specifiers: requests,
            peers,
            peer_requirements: info.peer_dependencies.clone().into_iter().collect(),
            optional_peers: info
                .peer_dependencies_meta
                .iter()
                .filter(|(_, meta)| meta.optional)
                .map(|(name, _)| name.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            optional_dependencies: info
                .optional_dependencies
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            os: info.os.clone(),
            cpu: info.cpu.clone(),
            libc: info.libc.clone(),
        });
    }
    let mut local_views = BTreeMap::new();
    for package in &packages {
        if let Some(path) = &package.path {
            if local_views.insert(path.clone(), package.id()).is_some() {
                return Err(FerriteError::Npm(format!("local source {path} needs conflicting peer contexts; one live directory must have one dependency view")));
            }
        }
    }
    packages.sort_by_key(LockedPackage::id);
    Ok((
        packages,
        importers
            .into_iter()
            .map(|(path, roots)| {
                (
                    path,
                    roots
                        .into_iter()
                        .filter(|(name, _)| !name.starts_with('\0'))
                        .map(|(name, index)| (name, ids[&partition[index]].clone()))
                        .collect(),
                )
            })
            .collect(),
    ))
}
