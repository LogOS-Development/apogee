#!/usr/bin/env python3
"""Generate tests/fixtures/igrf13_reference.csv from ppigrf.

Requires: pip install ppigrf pandas

The fixture uses geocentric spherical components:
  Br   — outward positive
  Bθ   — southward positive
  Bφ   — eastward positive

The fixture is intentionally approximate (spherical Earth radius = IGRF
reference radius + altitude). It is used only for regression testing that the
Apogee IGRF-13 implementation does not drift from the independent reference.
"""

import csv
from pathlib import Path

import pandas as pd
import ppigrf

CASES = [
    (0.0, 0.0, 400.0, "2020-01-01"),
    (45.0, 0.0, 400.0, "2020-01-01"),
    (-45.0, 120.0, 400.0, "2020-01-01"),
    (80.0, -60.0, 0.0, "2020-01-01"),
    (0.0, 0.0, 400.0, "2024-01-01"),
    (45.0, 0.0, 400.0, "2024-01-01"),
    (-80.0, 30.0, 200.0, "2022-06-15"),
    (30.0, -120.0, 800.0, "2022-06-15"),
]


def main() -> None:
    rows = []
    for lat, lon, alt, date in CASES:
        r = 6371.2 + alt
        theta = 90.0 - lat
        phi = lon
        br, btheta, bphi = ppigrf.igrf_gc(r, theta, phi, pd.Timestamp(date))
        rows.append(
            {
                "lat_deg": lat,
                "lon_deg": lon,
                "alt_km": alt,
                "date": date,
                # ppigrf uses inward-positive radial; convert to outward positive.
                "Br_nT": -float(br),
                "Btheta_nT": float(btheta),
                "Bphi_nT": float(bphi),
            }
        )

    fixture = Path(__file__).resolve().parents[1] / "tests" / "fixtures" / "igrf13_reference.csv"
    fixture.parent.mkdir(parents=True, exist_ok=True)
    with open(fixture, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    print(f"wrote {fixture}")


if __name__ == "__main__":
    main()
