export function sameProcess(before, after) {
  for (const row of [before, after]) {
    if (!Number.isInteger(row.ProcessId) || row.ProcessId <= 0 ||
        typeof row.Created !== 'string' || !/^\d+$/.test(row.Created))
      throw new Error('Windows process identity unavailable');
  }
  return before.ProcessId === after.ProcessId && before.Created === after.Created;
}
