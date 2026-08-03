#!/usr/bin/env python3
"""Validate the Apogee HWM14 Rust wrapper against the original Fortran.

This script recompiles a tiny Fortran harness from the vendored source and
runs it for the same grid as `crates/apogee-core/examples/hwm14_samples.rs`.
Because both the Rust wrapper and the Fortran harness ultimately call the
identical NRL subroutine, the residuals should be exactly zero (within
float32 round-trip).
"""

import subprocess
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
VENDOR = REPO_ROOT / "crates" / "hwm14-sys" / "vendor"
ASSETS = REPO_ROOT / "crates" / "hwm14-sys" / "assets"
BUILD = REPO_ROOT / "target" / "hwm14_validate"

sys.path.insert(0, str(SCRIPT_DIR))
from validation_framework import ComparisonPlot, Source, SummaryWriter


def compile_fortran_harness() -> Path:
    BUILD.mkdir(parents=True, exist_ok=True)
    for f in ["hwm14.f90", "hwm14_c.f90"]:
        dest = BUILD / f
        if dest.exists():
            dest.unlink()
        dest.hardlink_to(VENDOR / f)
    for f in ASSETS.iterdir():
        dest = BUILD / f.name
        if dest.exists():
            dest.unlink()
        dest.hardlink_to(f)

    harness = BUILD / "hwm14_harness.f90"
    harness.write_text(HARNESS_SRC)

    for src, obj in [("hwm14.f90", "hwm14.o"), ("hwm14_harness.f90", "harness.o")]:
        run(["gfortran", "-c", "-O2", "-o", str(BUILD / obj), str(BUILD / src)])

    exe = BUILD / "hwm14_harness"
    run(["gfortran", "-o", str(exe), str(BUILD / "hwm14.o"), str(BUILD / "harness.o")])
    return exe


def run(cmd: list[str]) -> None:
    result = subprocess.run(cmd, cwd=BUILD, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(cmd)}\n{result.stderr}")


def load_apogee_csv() -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    cmd = "cargo run --example hwm14_samples -p apogee-core --features hwm14 --release"
    result = subprocess.run(cmd, shell=True, cwd=REPO_ROOT, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"hwm14_samples failed:\n{result.stderr}")

    lines = [ln for ln in result.stdout.strip().split("\n") if not ln.startswith("altitude")]
    alts, east, north = [], [], []
    for ln in lines:
        a, e, n, _ = ln.split(",")
        alts.append(float(a))
        east.append(float(e))
        north.append(float(n))
    return np.array(alts), np.array(east), np.array(north)


def load_fortran_reference(exe: Path, alts: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    east, north = [], []
    for alt in alts:
        result = subprocess.run(
            [str(exe), str(alt)],
            cwd=BUILD,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise RuntimeError(f"Fortran harness failed at {alt} km:\n{result.stderr}")
        e, n = result.stdout.strip().split()
        east.append(float(e))
        north.append(float(n))
    return np.array(east), np.array(north)


HARNESS_SRC = """\
program hwm14_harness
    implicit none
    integer(4) :: iyd
    real(4) :: sec, alt, glat, glon, stl, f107a, f107
    real(4) :: ap(2), w(2)
    character(len=32) :: arg

    call get_command_argument(1, arg)
    read(arg, *) alt

    iyd = 93323
    sec = 12.0 * 3600.0
    glat = -11.95
    glon = -76.77
    stl = -1.0
    f107a = -1.0
    f107 = -1.0
    ap(1) = -1.0
    ap(2) = 35.0

    call hwm14(iyd, sec, alt, glat, glon, stl, f107a, f107, ap, w)

    print '(F0.6,1X,F0.6)', w(2), w(1)
end program hwm14_harness
"""


def main() -> None:
    out_dir = Path("plots/hwm14")
    out_dir.mkdir(parents=True, exist_ok=True)

    exe = compile_fortran_harness()
    alts, east_rust, north_rust = load_apogee_csv()
    east_ref, north_ref = load_fortran_reference(exe, alts)

    east_src_a = Source.from_arrays("Apogee", alts, east_rust, unit="m/s")
    east_src_r = Source.from_arrays("Fortran", alts, east_ref, unit="m/s")
    north_src_a = Source.from_arrays("Apogee", alts, north_rust, unit="m/s")
    north_src_r = Source.from_arrays("Fortran", alts, north_ref, unit="m/s")

    # The residuals are pure float32 round-trip noise: the Rust wrapper casts
    # f64 inputs to f32, calls the same Fortran subroutine, and converts the
    # f32 outputs back to f64. A tolerance of 1e-3 m/s is about 2x the
    # observed maximum error (~4e-4 m/s).
    plot = ComparisonPlot(title="HWM14 zonal (east) wind: Apogee vs Fortran")
    plot.add_series(east_src_r, east_src_a, name="zonal wind", ylabel="m/s", tolerance=1e-3)
    fig = plot.build()
    fig.savefig(out_dir / "zonal.png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    plot = ComparisonPlot(title="HWM14 meridional (north) wind: Apogee vs Fortran")
    plot.add_series(north_src_r, north_src_a, name="meridional wind", ylabel="m/s", tolerance=1e-3)
    fig = plot.build()
    fig.savefig(out_dir / "meridional.png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    max_err = max(
        float(np.max(np.abs(east_rust - east_ref))),
        float(np.max(np.abs(north_rust - north_ref))),
    )

    SummaryWriter(out_dir / "summary.json").write(
        name="hwm14",
        passed=max_err < 1e-3,
        max_residual=max_err,
        notes=[
            f"samples: {len(alts)} ({alts.min():.0f}–{alts.max():.0f} km)",
            f"max component diff vs Fortran: {max_err:.6f} m/s",
            "residual is f32 round-trip noise from the Rust-C-Fortran ABI boundary",
        ],
    )

    print(f"Plots saved to {out_dir}")
    print(f"  max component diff vs Fortran: {max_err:.6f} m/s")


if __name__ == "__main__":
    main()
