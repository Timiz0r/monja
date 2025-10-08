use googletest::prelude::*;
use monja::{MonjaProfile, SetName};

use crate::sim::Simulator;

pub mod sim;

// TODO: index verification

#[gtest]
fn simple_set() {
    let mut sim = Simulator::create();
    sim = sim.configure_profile(|old| MonjaProfile {
        target_sets: set_names(["simple"]),
        ..old
    });

    #[rustfmt::skip]
    sim.set(SetName("simple".into()), |s| _ = s
        .dir("foo", |d| _ = d
            .dir("bar/baz", |d| _ = d
                .file("a", "a")))
        .dir("apple", |d| _ = d
            .file("pie", "pie"))
        .file("blueberry", "tart"));

    monja::pull(sim.profile());

    #[rustfmt::skip]
    sim.validate_set(SetName("simple".into()), |s| _ = s
        .validate_dir("foo", |d| _ = d
            .validate_dir("bar/baz", |d| _ = d
                .validate_file("a", "a")))
        .validate_dir("apple", |d| _ = d
            .validate_file("pie", "pie"))
        .validate_file("blueberry", "tart"));
}

#[gtest]
fn multiple_sets() {}

#[gtest]
fn shortcuts() {}

#[gtest]
fn missing_set() {}

#[gtest]
fn directory_traversal() {}

fn set_names<S, N>(names: N) -> Vec<SetName>
where
    S: AsRef<str>,
    N: AsRef<[S]>,
{
    names
        .as_ref()
        .iter()
        .map(|n| SetName(n.as_ref().into()))
        .collect()
}
