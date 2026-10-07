import type { Sample } from './contracts';

// 显示网格和绘图抽样只控制呈现成本，不改写原始科研样本或验证门槛。
export const DISPLAY_LIMITS = {
  radialSegments: 96, angularSegments: 64, maxLineSamples: 2000,
  wireRadialSegments: 16, wireAngularSegments: 16,
  frameIntervalMs: 40, secondsPerPlayback: 12,
} as const;
export type Point3 = [number, number, number];

// 赤道嵌入曲面满足 R²=l²+a²、z=a asinh(l/a)；嵌入轴放在屏幕场景的 x 轴。
export function embeddingPoint(l: number, phi: number, throatRadius: number): Point3 {
  const radius = Math.hypot(l, throatRadius);
  const ratio = l / throatRadius;
  const embeddingAxis = Number.isFinite(ratio) ? throatRadius * Math.asinh(ratio)
    : Math.sign(l) * throatRadius * (Math.log(Math.abs(l)) - Math.log(throatRadius) + Math.LN2);
  return [embeddingAxis, radius * Math.cos(phi), radius * Math.sin(phi)];
}

export function makeSurface(throatRadius: number, extent: number, cut: boolean, displayScale = 1) {
  const positions: number[] = [];
  const indices: number[] = [];
  const wireIndices: number[] = [];
  const { radialSegments, angularSegments, wireRadialSegments, wireAngularSegments } = DISPLAY_LIMITS;
  const angle = cut ? Math.PI : Math.PI * 2;
  for (let radial = 0; radial <= radialSegments; radial += 1) {
    const l = -extent + (2 * extent * radial) / radialSegments;
    for (let angular = 0; angular <= angularSegments; angular += 1) {
      positions.push(...embeddingPoint(l, angle * angular / angularSegments, throatRadius).map((coordinate) => coordinate / displayScale));
    }
  }
  for (let radial = 0; radial < radialSegments; radial += 1) {
    for (let angular = 0; angular < angularSegments; angular += 1) {
      const first = radial * (angularSegments + 1) + angular;
      const next = first + angularSegments + 1;
      indices.push(first, next, first + 1, next, next + 1, first + 1);
    }
  }
  // 线框只选取原曲面的经纬边；保持高分辨率三角面，避免绘制密集三角对角线。
  const rowStride = angularSegments + 1;
  const radialStep = radialSegments / wireRadialSegments;
  const longitudeCount = cut ? wireAngularSegments / 2 : wireAngularSegments;
  const angularStep = angularSegments / longitudeCount;
  for (let radial = 0; radial <= radialSegments; radial += radialStep) {
    for (let angular = 0; angular < angularSegments; angular += 1) {
      const first = radial * rowStride + angular;
      wireIndices.push(first, first + 1);
    }
  }
  // 完整曲面省去与0重合的2π经线；剖切则保留0、π两条开口边界。
  const lastLongitude = cut ? angularSegments : angularSegments - angularStep;
  for (let angular = 0; angular <= lastLongitude; angular += angularStep) {
    for (let radial = 0; radial < radialSegments; radial += 1) {
      const first = radial * rowStride + angular;
      wireIndices.push(first, first + rowStride);
    }
  }
  return { positions, indices, wireIndices };
}

// 二分定位后线性插值仅用于光标；表格和导出始终保留原始点。
export function sampleAtAffine(samples: Sample[], affine: number): Sample | null {
  if (samples.length === 0) return null;
  if (affine <= samples[0].affine) return samples[0];
  if (affine >= samples[samples.length - 1].affine) return samples[samples.length - 1];
  let low = 0;
  let high = samples.length - 1;
  while (high - low > 1) {
    const middle = Math.floor((low + high) / 2);
    if (samples[middle].affine <= affine) low = middle;
    else high = middle;
  }
  const left = samples[low];
  const right = samples[high];
  const weight = (affine - left.affine) / (right.affine - left.affine);
  const interpolate = (key: keyof Sample) => left[key] + (right[key] - left[key]) * weight;
  return {
    affine, t: interpolate('t'), l: interpolate('l'), theta: interpolate('theta'), phi: interpolate('phi'),
    kt: interpolate('kt'), kl: interpolate('kl'), kTheta: interpolate('kTheta'), kPhi: interpolate('kPhi'),
  };
}

// SCI07 归一化约定 E₀=1，L₀=b；b=0 时仍以 a×|E₀| 归一，避免除零。
export function residualSeries(samples: Sample[], throatRadius: number, impactParameter: number) {
  return samples.map((sample) => {
    const radiusSquared = sample.l ** 2 + throatRadius ** 2;
    const sineSquared = Math.sin(sample.theta) ** 2;
    return {
      affine: sample.affine,
      energyError: Math.abs(sample.kt - 1),
      angularMomentumError: Math.abs(radiusSquared * sineSquared * sample.kPhi - impactParameter) / throatRadius,
      nullError: Math.abs(-(sample.kt ** 2) + sample.kl ** 2 + radiusSquared * (sample.kTheta ** 2 + sineSquared * sample.kPhi ** 2)),
    };
  });
}

export function timeBounds(trajectories: Sample[][]): [number, number] {
  let maximum = 0;
  for (const samples of trajectories) {
    if (samples.length > 0) maximum = Math.max(maximum, samples[samples.length - 1].affine);
  }
  return [0, maximum];
}

export function advanceTimeline(affine: number, elapsedSeconds: number, rate: number, maximum: number) {
  const next = Math.min(maximum, Math.max(0, affine + elapsedSeconds * rate));
  return { affine: next, playing: next < maximum };
}

export function displaySamples(samples: Sample[], affine = Number.POSITIVE_INFINITY) {
  const visible = samples.filter((sample) => sample.affine <= affine);
  // 为插值端点预留一个名额，确保实际输出点数不超过显示预算。
  const stride = Math.max(1, Math.ceil(visible.length / (DISPLAY_LIMITS.maxLineSamples - 1)));
  const reduced = visible.filter((_sample, index) => index % stride === 0);
  const endpoint = sampleAtAffine(samples, affine);
  if (endpoint && reduced[reduced.length - 1]?.affine !== endpoint.affine) reduced.push(endpoint);
  return reduced;
}
