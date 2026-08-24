use plottypus_core::{ClusterKind, SensorsSnapshot, TempReading};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TempZone {
    Efficiency,
    Performance,
    Super,
    Gpu,
    Cpu,
    Other,
}

pub(crate) fn hid_has_macc<I, S>(names: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    names
        .into_iter()
        .any(|n| n.as_ref().to_ascii_lowercase().contains("macc"))
}

pub(crate) fn hid_temp_zone(name: &str, has_macc: bool) -> TempZone {
    let p = name.to_ascii_lowercase();
    if p.contains("eacc") {
        TempZone::Efficiency
    } else if p.contains("macc") {
        TempZone::Performance
    } else if p.contains("sacc") {
        TempZone::Super
    } else if p.contains("pacc") {
        if has_macc {
            TempZone::Super
        } else {
            TempZone::Performance
        }
    } else if p.contains("gpu") {
        TempZone::Gpu
    } else if p.contains("pmu tdie")
        || p.contains("soc mtr")
        || p.contains("pmgr")
        || p.contains("cpu")
    {
        TempZone::Cpu
    } else {
        TempZone::Other
    }
}

pub(crate) fn smc_temp_zone(key: &str, has_te: bool) -> TempZone {
    if key.starts_with("Te") {
        TempZone::Efficiency
    } else if key.starts_with("Tf") {
        TempZone::Performance
    } else if key.starts_with("Tp") {
        if has_te {
            TempZone::Performance
        } else {
            TempZone::Cpu
        }
    } else if key.starts_with("TC") {
        TempZone::Cpu
    } else if key.starts_with("Tg") || key.starts_with("TG") {
        TempZone::Gpu
    } else {
        TempZone::Other
    }
}

pub(crate) fn smc_has_te<I, S>(keys: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    keys.into_iter().any(|k| k.as_ref().starts_with("Te"))
}

/// `IORegistry` `cluster-type` letter. `P` is Super when `M` exists or a Super perflevel exists without an `S` letter.
pub(crate) fn cluster_letter_kind(
    letter: char,
    letters: &str,
    has_super_level: bool,
) -> Option<ClusterKind> {
    match letter.to_ascii_uppercase() {
        'E' => Some(ClusterKind::Efficiency),
        'S' => Some(ClusterKind::Super),
        'P' if letters.contains('M') || (has_super_level && !letters.contains('S')) => {
            Some(ClusterKind::Super)
        }
        'M' | 'P' => Some(ClusterKind::Performance),
        _ => None,
    }
}

pub(crate) fn kind_from_level_name(name: &str) -> Option<ClusterKind> {
    let n = name.to_ascii_lowercase();
    if n.contains("super") {
        Some(ClusterKind::Super)
    } else if n.contains("efficien") {
        Some(ClusterKind::Efficiency)
    } else if n.contains("perform") {
        Some(ClusterKind::Performance)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Hid,
    Smc,
}

pub(crate) fn snapshot_from_named(named: &[(String, f32)], source: Source) -> SensorsSnapshot {
    let has_macc = hid_has_macc(named.iter().map(|(n, _)| n.as_str()));
    let has_te = smc_has_te(named.iter().map(|(n, _)| n.as_str()));
    let mut e = Vec::new();
    let mut p = Vec::new();
    let mut s = Vec::new();
    let mut cpu = Vec::new();
    let mut gpu = Vec::new();
    let mut extras = Vec::new();
    let mut hotspot = None;
    for (name, temp) in named {
        if !temp.is_finite() || *temp <= 0.0 || *temp > 150.0 {
            continue;
        }
        hotspot = Some(hotspot.map_or(*temp, |h: f32| h.max(*temp)));
        let zone = match source {
            Source::Hid => hid_temp_zone(name, has_macc),
            Source::Smc => smc_temp_zone(name, has_te),
        };
        match zone {
            TempZone::Efficiency => e.push(*temp),
            TempZone::Performance => p.push(*temp),
            TempZone::Super => s.push(*temp),
            TempZone::Gpu => gpu.push(*temp),
            TempZone::Cpu => cpu.push(*temp),
            TempZone::Other => {
                if extras.len() < 6 {
                    extras.push(TempReading {
                        name: extra_name(name),
                        celsius: *temp,
                    });
                }
            }
        }
    }
    let mut snap = SensorsSnapshot {
        e_c: mean(&e),
        p_c: mean(&p),
        s_c: mean(&s),
        gpu_c: mean(&gpu),
        hotspot_c: hotspot,
        ..SensorsSnapshot::default()
    };
    snap.cpu_c = mean(&cpu).or_else(|| mean_opts(&[snap.e_c, snap.p_c, snap.s_c]));
    snap.readings = zone_readings(&snap);
    snap.readings.extend(extras);
    snap
}

pub(crate) fn merge_sensors(mut base: SensorsSnapshot, overlay: &SensorsSnapshot) -> SensorsSnapshot {
    if overlay.e_c.is_some() {
        base.e_c = overlay.e_c;
    }
    if overlay.p_c.is_some() {
        base.p_c = overlay.p_c;
    }
    if overlay.s_c.is_some() {
        base.s_c = overlay.s_c;
    }
    if base.cpu_c.is_none() {
        base.cpu_c = overlay.cpu_c;
    }
    if base.gpu_c.is_none() {
        base.gpu_c = overlay.gpu_c;
    }
    base.hotspot_c = match (base.hotspot_c, overlay.hotspot_c) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    if base.cpu_c.is_none() {
        base.cpu_c = mean_opts(&[base.e_c, base.p_c, base.s_c]);
    }
    let extras: Vec<TempReading> = base
        .readings
        .iter()
        .chain(overlay.readings.iter())
        .filter(|r| {
            !matches!(
                r.name.as_str(),
                "cpu" | "gpu" | "efficiency" | "performance" | "super"
            )
        })
        .take(6)
        .cloned()
        .collect();
    base.readings = zone_readings(&base);
    for extra in extras {
        if !base.readings.iter().any(|r| r.name == extra.name) {
            base.readings.push(extra);
        }
    }
    base
}

fn zone_readings(snap: &SensorsSnapshot) -> Vec<TempReading> {
    let mut out = Vec::new();
    push_reading(&mut out, "efficiency", snap.e_c);
    push_reading(&mut out, "performance", snap.p_c);
    push_reading(&mut out, "super", snap.s_c);
    if snap.e_c.is_none() && snap.p_c.is_none() && snap.s_c.is_none() {
        push_reading(&mut out, "cpu", snap.cpu_c);
    }
    push_reading(&mut out, "gpu", snap.gpu_c);
    out
}

fn push_reading(out: &mut Vec<TempReading>, name: &str, temp: Option<f32>) {
    if let Some(celsius) = temp {
        out.push(TempReading {
            name: name.to_owned(),
            celsius,
        });
    }
}

fn extra_name(name: &str) -> String {
    let p = name.to_ascii_lowercase();
    if p.contains("nand") || name.starts_with("TH") {
        String::from("nand")
    } else if p.contains("ambient") || name.starts_with("TA") || name.starts_with("Ta") {
        String::from("ambient")
    } else if name.is_empty() {
        String::from("temp")
    } else {
        name.chars().take(12).collect()
    }
}

fn mean(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f32>() / values.len() as f32)
    }
}

fn mean_opts(values: &[Option<f32>]) -> Option<f32> {
    let present: Vec<f32> = values.iter().copied().flatten().collect();
    mean(&present)
}

pub(crate) fn counts_from_levels(levels: &[(ClusterKind, u8)]) -> (u8, u8, u8) {
    let mut e = 0_u8;
    let mut p = 0_u8;
    let mut s = 0_u8;
    for (kind, n) in levels {
        match kind {
            ClusterKind::Efficiency => e = e.saturating_add(*n),
            ClusterKind::Performance => p = p.saturating_add(*n),
            ClusterKind::Super => s = s.saturating_add(*n),
        }
    }
    (e, p, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_acc_names() {
        assert_eq!(
            hid_temp_zone("eACC MTR Temp Sensor1", false),
            TempZone::Efficiency
        );
        assert_eq!(
            hid_temp_zone("pACC MTR Temp Sensor2", false),
            TempZone::Performance
        );
        assert_eq!(
            hid_temp_zone("pACC MTR Temp Sensor2", true),
            TempZone::Super
        );
        assert_eq!(hid_temp_zone("mACC MTR Temp Sensor0", true), TempZone::Performance);
        assert_eq!(hid_temp_zone("PMU tdie3", false), TempZone::Cpu);
        assert_eq!(hid_temp_zone("GPU MTR Temp Sensor1", false), TempZone::Gpu);
        assert_eq!(hid_temp_zone("NAND CH0 temp", false), TempZone::Other);
        assert!(hid_has_macc(["pACC", "mACC 1"]));
        assert!(!hid_has_macc(["pACC", "eACC"]));
    }

    #[test]
    fn smc_keys_split_only_when_honest() {
        assert_eq!(smc_temp_zone("Te05", true), TempZone::Efficiency);
        assert_eq!(smc_temp_zone("Tf04", true), TempZone::Performance);
        assert_eq!(smc_temp_zone("Tp01", true), TempZone::Performance);
        assert_eq!(smc_temp_zone("Tp01", false), TempZone::Cpu);
        assert_eq!(smc_temp_zone("TC0C", false), TempZone::Cpu);
        assert_eq!(smc_temp_zone("Tg0D", false), TempZone::Gpu);
        assert!(smc_has_te(["Tp01", "Te05"]));
        assert!(!smc_has_te(["Tp01", "TC0P"]));
    }

    #[test]
    fn cluster_letters() {
        assert_eq!(
            cluster_letter_kind('E', "EP", false),
            Some(ClusterKind::Efficiency)
        );
        assert_eq!(
            cluster_letter_kind('P', "EP", false),
            Some(ClusterKind::Performance)
        );
        assert_eq!(
            cluster_letter_kind('M', "MP", true),
            Some(ClusterKind::Performance)
        );
        assert_eq!(
            cluster_letter_kind('P', "MP", true),
            Some(ClusterKind::Super)
        );
        assert_eq!(
            cluster_letter_kind('P', "EP", true),
            Some(ClusterKind::Super)
        );
        assert_eq!(
            cluster_letter_kind('P', "EPS", true),
            Some(ClusterKind::Performance)
        );
        assert_eq!(cluster_letter_kind('X', "EP", false), None);
    }

    #[test]
    fn named_perflevels() {
        assert_eq!(kind_from_level_name("Super"), Some(ClusterKind::Super));
        assert_eq!(
            kind_from_level_name("Performance"),
            Some(ClusterKind::Performance)
        );
        assert_eq!(
            kind_from_level_name("Efficiency"),
            Some(ClusterKind::Efficiency)
        );
        assert_eq!(
            counts_from_levels(&[
                (ClusterKind::Super, 6),
                (ClusterKind::Performance, 12),
            ]),
            (0, 12, 6)
        );
    }

    #[test]
    fn hid_snapshot_averages_zones_not_tdie() {
        let snap = snapshot_from_named(
            &[
                (String::from("eACC MTR 1"), 34.0),
                (String::from("eACC MTR 2"), 38.0),
                (String::from("pACC MTR 1"), 50.0),
                (String::from("PMU tdie3"), 90.0),
                (String::from("GPU MTR 1"), 47.0),
            ],
            Source::Hid,
        );
        assert_eq!(snap.e_c, Some(36.0));
        assert_eq!(snap.p_c, Some(50.0));
        assert_eq!(snap.gpu_c, Some(47.0));
        assert_eq!(snap.cpu_c, Some(90.0));
        assert!(snap.readings.iter().any(|r| r.name == "efficiency"));
        assert!(snap.readings.iter().any(|r| r.name == "performance"));
        assert!(!snap.readings.iter().any(|r| r.name.contains("tdie")));
    }

    #[test]
    fn smc_tp_stays_package_without_te() {
        let snap = snapshot_from_named(
            &[
                (String::from("Tp01"), 44.0),
                (String::from("Tp05"), 46.0),
                (String::from("Tg0D"), 51.0),
            ],
            Source::Smc,
        );
        assert_eq!(snap.e_c, None);
        assert_eq!(snap.p_c, None);
        assert_eq!(snap.cpu_c, Some(45.0));
        assert_eq!(snap.gpu_c, Some(51.0));
    }

    #[test]
    fn merge_prefers_hid_zones() {
        let smc = snapshot_from_named(
            &[(String::from("Tp01"), 40.0), (String::from("TH0x"), 33.0)],
            Source::Smc,
        );
        let hid = snapshot_from_named(
            &[
                (String::from("eACC 1"), 35.0),
                (String::from("pACC 1"), 49.0),
            ],
            Source::Hid,
        );
        let merged = merge_sensors(smc, &hid);
        assert_eq!(merged.e_c, Some(35.0));
        assert_eq!(merged.p_c, Some(49.0));
        assert_eq!(merged.cpu_c, Some(40.0));
        assert!(merged.readings.iter().any(|r| r.name == "nand"));
    }
}
