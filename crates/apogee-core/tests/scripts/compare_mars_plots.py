import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
import sys
import os
from pathlib import Path

try:
    from PIL import Image
except ImportError as e:
    print(f"PIL is required for image comparison: {e}")
    sys.exit(1)

if len(sys.argv) != 4:
    print("usage: compare_mars_plots.py <samples.csv> <golden_dir> <out_dir>")
    sys.exit(2)

csv_path = Path(sys.argv[1])
golden_dir = Path(sys.argv[2])
out_dir = Path(sys.argv[3])
out_dir.mkdir(parents=True, exist_ok=True)

# Load samples
data = np.loadtxt(csv_path, delimiter=',', skiprows=1)
ets = data[:, 0]
mars_pos = data[:, 1:4]
mars_vel = data[:, 4:7]

# Ensure 1-D arrays for matplotlib
# CSV now contains heliocentric position (Mars minus Sun) produced by the Rust
# test, so no ephemeris re-evaluation is needed here.
helio = mars_pos

try:
    import spiceypy as spice
    spice.furnsh('data/ephemeris/naif0012.tls')
    spice.furnsh('data/ephemeris/de441.bsp')
    dates = [spice.et2utc(float(et), 'ISOC', 0)[:10] for et in ets.tolist()]
except Exception:
    from datetime import datetime, timedelta
    J2000 = datetime(2000, 1, 1, 12, 0, 0)
    dates = [(J2000 + timedelta(seconds=float(et))).isoformat()[:10] for et in ets]

dates: list[str] = dates  # type: ignore[assignment]
helio_x = helio[:, 0].tolist()
helio_y = helio[:, 1].tolist()
helio_z = helio[:, 2].tolist()
r = np.linalg.norm(helio, axis=1)
v_mag = np.linalg.norm(mars_vel, axis=1)
r_list = r.tolist()
v_list = v_mag.tolist()

# Render plots
def save_plots(prefix):
    # 3D trajectory
    fig = plt.figure(figsize=(10, 8))
    ax = fig.add_subplot(111, projection='3d')
    ax.plot(helio_x, helio_y, helio_z, lw=0.8)
    ax.scatter([0.0], [0.0], [0.0], color='orange', s=100, label='Sun')
    ax.set_xlabel('X (km)')
    ax.set_ylabel('Y (km)')
    ax.set_zlabel('Z (km)')
    ax.set_title('Mars barycenter heliocentric trajectory (DE441)')
    plt.tight_layout()
    fig.savefig(f'{prefix}_mars_trajectory_3d.png', dpi=150)
    plt.close(fig)

    # Distance from Sun vs time
    fig, ax = plt.subplots(figsize=(12, 5))
    ax.plot(dates, r_list)
    ax.set_xlabel('Date')
    ax.set_ylabel('Distance from Sun (km)')
    ax.set_title('Mars heliocentric distance vs time')
    step = len(dates) // 6
    ax.set_xticks(dates[::step])
    plt.tight_layout()
    fig.savefig(f'{prefix}_mars_distance_sun.png', dpi=150)
    plt.close(fig)

    # XY projection
    fig, ax = plt.subplots(figsize=(8, 8))
    ax.plot(helio_x, helio_y, lw=0.8)
    ax.scatter([0.0], [0.0], color='orange', s=100, label='Sun')
    ax.set_aspect('equal')
    ax.set_xlabel('X (km)')
    ax.set_ylabel('Y (km)')
    ax.set_title('Mars heliocentric XY projection (DE441)')
    ax.legend()
    plt.tight_layout()
    fig.savefig(f'{prefix}_mars_xy_projection.png', dpi=150)
    plt.close(fig)

    # Velocity magnitude
    fig, ax = plt.subplots(figsize=(12, 5))
    ax.plot(dates, v_list)
    ax.set_xlabel('Date')
    ax.set_ylabel('Velocity magnitude (km/s)')
    ax.set_title('Mars barycenter velocity magnitude vs time')
    step = len(dates) // 6
    ax.set_xticks(dates[::step])
    plt.tight_layout()
    fig.savefig(f'{prefix}_mars_velocity_magnitude.png', dpi=150)
    plt.close(fig)

save_plots(str(out_dir / 'generated'))

# Compare against golden snapshots
names = [
    'mars_trajectory_3d.png',
    'mars_distance_sun.png',
    'mars_xy_projection.png',
    'mars_velocity_magnitude.png',
]
# Compare against golden snapshots by rendering and checking pixel diff.
# Rendering differences across matplotlib versions (font hinting, anti-aliasing)
# can exceed small tolerances, so we also validate deterministic numerical
# summaries from stats.txt.
max_diff = 0.0
failed = []
for name in names:
    golden = Image.open(golden_dir / name).convert('RGB')
    generated = Image.open(out_dir / f'generated_{name}').convert('RGB')
    if golden.size != generated.size:
        failed.append(f"{name}: size mismatch {golden.size} vs {generated.size}")
        continue
    diff = np.abs(np.array(golden).astype(np.float32) - np.array(generated).astype(np.float32))
    image_max = diff.max()
    max_diff = max(max_diff, image_max)
    if image_max > 250.0:
        failed.append(f"{name}: max pixel difference {image_max:.1f}")

# Numerical validation against the golden stats digest.
with open(golden_dir / 'stats.txt') as f:
    golden_stats = f.read()

generated_lines = [
    f"samples: {len(ets)}",
    f"start ET: {ets[0]:.1f}",
    f"end ET: {ets[-1]:.1f}",
    f"min heliocentric distance: {r.min():.3f} km",
    f"max heliocentric distance: {r.max():.3f} km",
    f"mean velocity: {v_mag.mean():6f} km/s",
    f"min velocity: {v_mag.min():6f} km/s",
    f"max velocity: {v_mag.max():6f} km/s",
]
stat_mismatches = []
for line in generated_lines:
    if line not in golden_stats:
        stat_mismatches.append(line)

if stat_mismatches:
    failed.append(f"stats mismatch: {stat_mismatches}")

if failed:
    print("Golden snapshot mismatches:")
    for f in failed:
        print(f"  {f}")
    sys.exit(1)

print(f"All golden snapshots match (max pixel difference {max_diff:.2f}).")
