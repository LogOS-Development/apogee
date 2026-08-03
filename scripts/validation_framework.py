#!/usr/bin/env python3
"""Apogee validation plotting framework.

Provides reusable helpers for comparing Apogee outputs against independent
reference sources (ERFA, SPICE, Horizons, etc.) in validation runs.

Core abstractions:
- `Source`: a labeled dataset with x, y arrays and units.
- `ComparisonPlot`: builds multi-panel figures with value and residual plots.
- `GoldenSnapshot`: compares generated plots against committed golden PNGs.

Usage:
    from validation_framework import Source, ComparisonPlot, GoldenSnapshot

    apogee = Source.from_arrays('Apogee', years, values, unit='arcsec')
    erfa = Source.from_arrays('ERFA', years, values, unit='arcsec')

    plot = ComparisonPlot(title='Nutation in longitude', ylabel='Δψ')
    plot.add_series(apogee, erfa)
    plot.save(out_dir / 'dpsi.png')
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, Sequence

import matplotlib
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.figure import Figure
from numpy.typing import ArrayLike

try:
    from PIL import Image
except ImportError as e:  # pragma: no cover
    raise ImportError("PIL is required for golden snapshot comparison") from e

matplotlib.use("Agg")


@dataclass(frozen=True)
class Source:
    """A labeled data source for validation comparisons."""

    name: str
    x: np.ndarray
    y: np.ndarray
    unit: str = ""
    color: str | None = None
    linestyle: str = "-"
    marker: str | None = None

    def __post_init__(self) -> None:
        if self.x.shape != self.y.shape:
            raise ValueError("x and y must have the same shape")

    @classmethod
    def from_arrays(
        cls,
        name: str,
        x: ArrayLike,
        y: ArrayLike,
        *,
        unit: str = "",
        color: str | None = None,
        linestyle: str = "-",
        marker: str | None = None,
    ) -> Source:
        return cls(
            name=name,
            x=np.asarray(x, dtype=float),
            y=np.asarray(y, dtype=float),
            unit=unit,
            color=color,
            linestyle=linestyle,
            marker=marker,
        )

    @classmethod
    def from_csv(
        cls,
        name: str,
        csv_path: Path,
        *,
        x_col: str,
        y_col: str,
        unit: str = "",
        color: str | None = None,
    ) -> Source:
        import pandas as pd

        df = pd.read_csv(csv_path)
        return cls.from_arrays(
            name, df[x_col].to_numpy(), df[y_col].to_numpy(), unit=unit, color=color
        )

    def label(self) -> str:
        return f"{self.name} [{self.unit}]" if self.unit else self.name


@dataclass
class SeriesPair:
    """A pair of sources to compare, plus comparison metadata."""

    reference: Source
    candidate: Source
    name: str
    ylabel: str = ""
    residual_unit: str = ""
    tolerance: float | None = None


@dataclass
class ComparisonPlot:
    """Builder for a multi-panel comparison figure.

    For each added series pair, creates two panels:
      - top: the two series overlaid
      - bottom: candidate minus reference (residual)

    The residual panel draws a zero line and, if `tolerance` is set, horizontal
    tolerance bands at ±tolerance.
    """

    title: str = "Validation comparison"
    figsize: tuple[float, float] = (10, 6)
    sharex: bool = True
    series: list[SeriesPair] = field(default_factory=list)

    def add_series(
        self,
        reference: Source,
        candidate: Source,
        *,
        name: str = "",
        ylabel: str = "",
        residual_unit: str = "",
        tolerance: float | None = None,
    ) -> ComparisonPlot:
        self.series.append(
            SeriesPair(
                reference=reference,
                candidate=candidate,
                name=name or f"{candidate.name} vs {reference.name}",
                ylabel=ylabel,
                residual_unit=residual_unit,
                tolerance=tolerance,
            )
        )
        return self

    def build(self) -> Figure:
        n = len(self.series)
        if n == 0:
            raise ValueError("No series added to plot")

        fig, axes = plt.subplots(
            nrows=2 * n,
            ncols=1,
            figsize=(self.figsize[0], self.figsize[1] * n),
            sharex=self.sharex,
            squeeze=False,
        )

        for i, pair in enumerate(self.series):
            ax_value = axes[2 * i, 0]
            ax_residual = axes[2 * i + 1, 0]

            ref = pair.reference
            cand = pair.candidate

            # Top panel: values
            ax_value.plot(
                ref.x, ref.y, label=ref.label(), color=ref.color or "C0", linestyle=ref.linestyle
            )
            ax_value.plot(
                cand.x,
                cand.y,
                label=cand.label(),
                color=cand.color or "C1",
                linestyle=cand.linestyle,
                marker=cand.marker,
                markevery=max(1, len(cand.x) // 20),
            )
            ax_value.set_ylabel(pair.ylabel or ref.unit)
            ax_value.set_title(pair.name)
            ax_value.legend(loc="best")
            ax_value.grid(True, linestyle="--", alpha=0.5)

            # Bottom panel: residual
            residual = cand.y - ref.y
            ax_residual.plot(cand.x, residual, color="C2", linewidth=1.2)
            ax_residual.axhline(0, color="black", linewidth=0.8, linestyle="--")
            if pair.tolerance is not None:
                ax_residual.axhline(pair.tolerance, color="red", linewidth=0.8, linestyle=":")
                ax_residual.axhline(-pair.tolerance, color="red", linewidth=0.8, linestyle=":")
            ax_residual.set_ylabel(f"Δ ({cand.name} − {ref.name}) [{pair.residual_unit or ref.unit}]")
            ax_residual.grid(True, linestyle="--", alpha=0.5)

        axes[-1, 0].set_xlabel(self.series[0].reference.x.shape[0] and "epoch" or "index")
        fig.suptitle(self.title, y=1.0)
        fig.tight_layout()
        return fig

    def save(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        fig = self.build()
        fig.savefig(path, dpi=150, bbox_inches="tight")
        plt.close(fig)


@dataclass
class GoldenSnapshot:
    """Compare generated plot PNGs against committed golden snapshots."""

    golden_dir: Path
    generated_dir: Path
    tolerance: float = 250.0

    def compare(self, names: Sequence[str]) -> tuple[float, list[str]]:
        max_diff = 0.0
        failures: list[str] = []
        for name in names:
            golden = self.golden_dir / name
            generated = self.generated_dir / name
            if not golden.exists():
                failures.append(f"{name}: missing golden snapshot {golden}")
                continue
            if not generated.exists():
                failures.append(f"{name}: missing generated plot {generated}")
                continue

            g_img = Image.open(golden).convert("RGB")
            gen_img = Image.open(generated).convert("RGB")
            if g_img.size != gen_img.size:
                failures.append(f"{name}: size mismatch {g_img.size} vs {gen_img.size}")
                continue

            diff = np.abs(
                np.array(g_img, dtype=np.float32) - np.array(gen_img, dtype=np.float32)
            )
            image_max = float(diff.max())
            max_diff = max(max_diff, image_max)
            if image_max > self.tolerance:
                failures.append(f"{name}: max pixel difference {image_max:.1f}")

        return max_diff, failures


@dataclass
class SummaryWriter:
    """Write a JSON summary of validation results for CI artifacts."""

    out_path: Path

    def write(
        self,
        *,
        name: str,
        passed: bool,
        max_residual: float | None = None,
        max_pixel_diff: float | None = None,
        notes: Iterable[str] = (),
    ) -> None:
        self.out_path.parent.mkdir(parents=True, exist_ok=True)
        summary = {
            "name": name,
            "passed": passed,
            "max_residual": max_residual,
            "max_pixel_diff": max_pixel_diff,
            "notes": list(notes),
        }
        with open(self.out_path, "w", encoding="utf-8") as f:
            json.dump(summary, f, indent=2)
            f.write("\n")


def main() -> None:
    """Small self-test: plot two synthetic sources and a residual."""
    x = np.linspace(0, 10, 100)
    ref = Source.from_arrays("Reference", x, np.sin(x), unit="rad")
    cand = Source.from_arrays("Candidate", x, np.sin(x) + 0.05 * np.cos(x), unit="rad")

    plot = ComparisonPlot(title="Framework self-test")
    plot.add_series(ref, cand, name="sin(x) comparison", ylabel="value", tolerance=0.1)

    out = Path("plots") / "validation_framework_selftest.png"
    plot.save(out)
    print(f"saved {out}")


if __name__ == "__main__":
    main()
