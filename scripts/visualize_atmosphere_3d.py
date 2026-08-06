#!/usr/bin/env python3
"""Render an animated 3D visualization of multi-model atmosphere data.

Usage:
    scripts/visualize_atmosphere_3d.py [alt_min_km] [alt_max_km] [frames]

The script:
  1. Runs `cargo run --example atmosphere_3d_grid -p apogee-core` to sample
     NRLMSISE-00 and Jacchia-Bowman density/temperature on a 3D grid.
  2. Builds a matplotlib 3D scatter + quiver animation of density spheres
     and wind vectors.
  3. Writes `plots/atmosphere_3d_animation.mp4` (requires ffmpeg).

If the `hwm14` feature is enabled, real HWM14 wind vectors are rendered;
otherwise the wind field is shown as zero (placeholder).
"""

import os
import subprocess
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import matplotlib.animation as animation
import numpy as np
from matplotlib.animation import FFMpegWriter
from mpl_toolkits.mplot3d import Axes3D  # noqa: F401


def run_grid_example(alt_min_km: float, alt_max_km: float) -> str:
    root = Path(__file__).resolve().parents[1]
    cmd = [
        "cargo",
        "run",
        "--example",
        "atmosphere_3d_grid",
        "-p",
        "apogee-core",
        "--",
        str(alt_min_km),
        str(alt_max_km),
    ]
    result = subprocess.run(
        cmd,
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )
    return result.stdout


def parse_csv(text: str) -> dict:
    lines = text.strip().splitlines()
    header = lines[0].split(",")
    rows = [ln.split(",") for ln in lines[1:]]
    out = {}
    for i, name in enumerate(header):
        if name == "model":
            out[name] = np.array([row[i] for row in rows])
        else:
            out[name] = np.array([float(row[i]) for row in rows])
    return out


def cartesian(lat_deg: np.ndarray, lon_deg: np.ndarray, alt_km: np.ndarray) -> tuple:
    r = 6371.0 + alt_km
    lat_rad = np.radians(lat_deg)
    lon_rad = np.radians(lon_deg)
    x = r * np.cos(lat_rad) * np.cos(lon_rad)
    y = r * np.cos(lat_rad) * np.sin(lon_rad)
    z = r * np.sin(lat_rad)
    return x, y, z


def main() -> None:
    alt_min_km = float(sys.argv[1]) if len(sys.argv) > 1 else 100.0
    alt_max_km = float(sys.argv[2]) if len(sys.argv) > 2 else 500.0
    n_frames = int(sys.argv[3]) if len(sys.argv) > 3 else 48

    print("Sampling atmosphere grid...")
    data = parse_csv(run_grid_example(alt_min_km, alt_max_km))

    models = np.unique(data["model"])
    print(f"Models: {models}")

    out_dir = Path(__file__).resolve().parents[1] / "plots"
    out_dir.mkdir(exist_ok=True)

    fig = plt.figure(figsize=(14, 7))
    axes: list[Axes3D] = []
    scatters: list = []
    quivers: list = []

    for idx, model in enumerate(models):
        ax = fig.add_subplot(1, len(models), idx + 1, projection="3d")
        axes.append(ax)
        ax.set_title(f"{model} — density + wind")
        ax.set_xlabel("x (km)")
        ax.set_ylabel("y (km)")
        ax.set_zlabel("z (km)")

        mask = data["model"] == model
        d = {k: v[mask] for k, v in data.items()}
        x, y, z = cartesian(d["lat_deg"], d["lon_deg"], d["alt_km"])

        # Density points: colour by altitude, size by density.
        rho = d["density_kg_m3"]
        sizes = np.clip((rho / rho.max()) * 200, 5, 200)
        scatter = ax.scatter(x, y, z, c=d["alt_km"], s=sizes, cmap="plasma", alpha=0.6)
        scatters.append(scatter)

        # Wind arrows.
        enu = np.column_stack((d["wind_east_mps"], d["wind_north_mps"], d["wind_up_mps"]))
        speed = np.linalg.norm(enu, axis=1)
        max_speed = speed.max()
        if max_speed > 0:
            scale = 300.0 / max_speed
        else:
            scale = 0.0

        # ENU to ECEF wind vector.
        lat_rad = np.radians(d["lat_deg"])
        lon_rad = np.radians(d["lon_deg"])
        up = np.column_stack(
            (np.cos(lat_rad) * np.cos(lon_rad),
             np.cos(lat_rad) * np.sin(lon_rad),
             np.sin(lat_rad))
        )
        east = np.column_stack((-np.sin(lon_rad), np.cos(lon_rad), np.zeros_like(lon_rad)))
        north = np.cross(up, east)
        world_wind = (
            east * d["wind_east_mps"][:, None]
            + north * d["wind_north_mps"][:, None]
            + up * d["wind_up_mps"][:, None]
        ) * scale

        quiver = ax.quiver(
            x, y, z,
            world_wind[:, 0], world_wind[:, 1], world_wind[:, 2],
            length=1.0, normalize=False, color="cyan", alpha=0.5, arrow_length_ratio=0.3,
        )
        quivers.append(quiver)

    # Animation: rotate the view around the Z axis.
    def update(frame: int):
        angle = frame * (360.0 / n_frames)
        for ax in axes:
            ax.view_init(elev=20.0, azim=angle)
        return scatters + quivers

    print(f"Rendering {n_frames} frames to {out_dir / 'atmosphere_3d_animation.mp4'}...")
    writer = FFMpegWriter(fps=12, metadata={"title": "Apogee multi-model atmosphere + wind"})
    anim = animation.FuncAnimation(fig, update, frames=n_frames, interval=1000 // 12, blit=False)
    anim.save(out_dir / "atmosphere_3d_animation.mp4", writer=writer)
    print("Done.")


if __name__ == "__main__":
    main()
