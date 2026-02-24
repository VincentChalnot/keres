'use strict';

self.onmessage = function (e) {
  const text = new TextDecoder().decode(e.data.buffer);

  // ── Parse JSONL ────────────────────────────────────────────────────────────
  const nodes = [];
  const nodeMap = new Map(); // node_id → array index

  for (const line of text.split('\n')) {
    const t = line.trim();
    if (!t) continue;
    try {
      const node = JSON.parse(t);
      nodeMap.set(node.node_id, nodes.length);
      nodes.push(node);
    } catch { /* skip malformed lines */ }
  }

  if (!nodes.length) {
    self.postMessage({ type: 'error', message: 'No nodes parsed from file.' });
    return;
  }

  // ── Build adjacency list ───────────────────────────────────────────────────
  const children = nodes.map(() => []);
  let rootIdx = -1;

  for (let i = 0; i < nodes.length; i++) {
    const n = nodes[i];
    if (n.parent_id === null || n.parent_id === undefined) {
      rootIdx = i;
    } else {
      const pi = nodeMap.get(n.parent_id);
      if (pi !== undefined) children[pi].push(i);
    }
  }

  if (rootIdx === -1) {
    self.postMessage({ type: 'error', message: 'No root node found (parent_id === null).' });
    return;
  }

  // ── Compute layout (normalized x in [0, 1]) ────────────────────────────────
  const nodeX = new Float64Array(nodes.length);
  const nodeW = new Float64Array(nodes.length);
  const nodeDepth = new Int32Array(nodes.length).fill(-1);
  const parentIdxs = new Int32Array(nodes.length).fill(-1);

  nodeX[rootIdx] = 0;
  nodeW[rootIdx] = 1;
  nodeDepth[rootIdx] = 0;

  // BFS – depthRows[d] = ordered list of node indices at depth d
  const depthRows = [[rootIdx]];
  const queue = [rootIdx];
  let qi = 0;

  while (qi < queue.length) {
    const idx = queue[qi++];
    const depth = nodeDepth[idx];
    const node = nodes[idx];
    const childList = children[idx];
    if (!childList.length) continue;

    // Sort best→worst for the player who chooses at this node:
    //   white_to_move → higher minimax_value first (desc)
    //   !white_to_move → lower minimax_value first (asc)
    const sorted = childList.slice().sort((a, b) => {
      const va = nodes[a].minimax_value ?? 0.5;
      const vb = nodes[b].minimax_value ?? 0.5;
      return node.white_to_move ? vb - va : va - vb;
    });

    const cw = nodeW[idx] / sorted.length;
    const cd = depth + 1;
    while (depthRows.length <= cd) depthRows.push([]);

    for (let i = 0; i < sorted.length; i++) {
      const ci = sorted[i];
      nodeX[ci] = nodeX[idx] + i * cw;
      nodeW[ci] = cw;
      nodeDepth[ci] = cd;
      parentIdxs[ci] = idx;
      depthRows[cd].push(ci);
      queue.push(ci);
    }
  }

  const maxDepth = depthRows.length - 1;

  // ── Metadata arrays ────────────────────────────────────────────────────────
  const nodeCount = nodes.length;
  const minimaxValues = new Float32Array(nodeCount);
  const whiteToMoves = new Uint8Array(nodeCount);
  const isTerminals = new Uint8Array(nodeCount);
  const nodeIds = new Int32Array(nodeCount);
  const actions = [];

  for (let i = 0; i < nodeCount; i++) {
    const n = nodes[i];
    minimaxValues[i] = n.minimax_value ?? 0.5;
    whiteToMoves[i] = n.white_to_move ? 1 : 0;
    isTerminals[i] = n.is_terminal ? 1 : 0;
    nodeIds[i] = n.node_id;
    actions.push(n.action ?? null);
  }

  // ── Per-row flat typed arrays (depth 1…maxDepth, root excluded) ───────────
  // xs are monotonically non-decreasing within each row (BFS left→right order)
  const rows = [];
  const transferables = [
    minimaxValues.buffer,
    whiteToMoves.buffer,
    isTerminals.buffer,
    nodeIds.buffer,
    parentIdxs.buffer,
  ];

  for (let d = 1; d <= maxDepth; d++) {
    const row = depthRows[d];
    const xs = new Float64Array(row.length);
    const ws = new Float64Array(row.length);
    const idxs = new Int32Array(row.length);
    for (let i = 0; i < row.length; i++) {
      xs[i] = nodeX[row[i]];
      ws[i] = nodeW[row[i]];
      idxs[i] = row[i];
    }
    rows.push({ depth: d, xs, ws, idxs });
    transferables.push(xs.buffer, ws.buffer, idxs.buffer);
  }

  const rootNode = nodes[rootIdx];
  self.postMessage({
    type: 'layout',
    rootData: {
      node_id: rootNode.node_id,
      minimax_value: rootNode.minimax_value,
      white_to_move: rootNode.white_to_move,
      is_terminal: rootNode.is_terminal,
    },
    rows,
    metadata: { minimaxValues, whiteToMoves, isTerminals, nodeIds, parentIdxs, actions },
    maxDepth,
    totalNodes: nodes.length,
  }, transferables);
};
