import test from 'node:test';
import assert from 'node:assert/strict';
import type { GraphNode } from '../types';
import { buildGroupStatusEntries, collapseGraphGroups } from './groupedGraphUtils.ts';

test('collapseGraphGroups only emits collapsed groups that exist in the current graph subset', () => {
  const nodes: GraphNode[] = [
    { id: 'a1', deps: [] },
    { id: 'a2', deps: ['a1'] },
    { id: 'b1', deps: ['a2'] },
    { id: 'x', deps: ['b1'] },
  ];

  const taskGroups = {
    groupA: ['a1', 'a2'],
    groupB: ['b1', 'b2'],
    groupC: ['c1'],
  };

  const collapsed = collapseGraphGroups(nodes, taskGroups, new Set(['groupA', 'groupB', 'groupC']));

  assert.deepEqual(collapsed, [
    { id: 'x', deps: ['__group__groupB'] },
    {
      id: '__group__groupA',
      deps: [],
      label: '▶ groupA (2)',
      exportLabel: 'groupA (2)',
    },
    {
      id: '__group__groupB',
      deps: ['__group__groupA'],
      label: '▶ groupB (1)',
      exportLabel: 'groupB (1)',
    },
  ]);
});

test('collapseGraphGroups leaves overlapping tasks expanded instead of arbitrarily assigning them', () => {
  const nodes: GraphNode[] = [
    { id: 'shared', deps: [] },
    { id: 'onlyA', deps: ['shared'] },
    { id: 'onlyB', deps: ['shared'] },
    { id: 'tail', deps: ['onlyA', 'onlyB'] },
  ];

  const taskGroups = {
    groupA: ['shared', 'onlyA'],
    groupB: ['shared', 'onlyB'],
  };

  const collapsed = collapseGraphGroups(nodes, taskGroups, new Set(['groupA', 'groupB']));

  assert.deepEqual(collapsed, [
    { id: 'shared', deps: [] },
    { id: 'tail', deps: ['__group__groupA', '__group__groupB'] },
    {
      id: '__group__groupA',
      deps: ['shared'],
      label: '▶ groupA (1)',
      exportLabel: 'groupA (1)',
    },
    {
      id: '__group__groupB',
      deps: ['shared'],
      label: '▶ groupB (1)',
      exportLabel: 'groupB (1)',
    },
  ]);
});

test('buildGroupStatusEntries uses only visible collapsed members and treats skipped as terminal', () => {
  const nodes: GraphNode[] = [
    { id: 'a1', deps: [] },
    { id: 'b1', deps: [] },
    { id: 'shared', deps: [] },
    { id: 'onlyA', deps: [] },
    { id: 'onlyB', deps: [] },
  ];

  const taskGroups = {
    groupA: ['a1', 'missing', 'shared', 'onlyA'],
    groupB: ['b1', 'shared', 'onlyB'],
  };

  const statuses = buildGroupStatusEntries(
    nodes,
    {
      a1: 'skipped',
      b1: 'pending',
      shared: 'failed',
      onlyA: 'success',
      onlyB: 'running',
      missing: 'failed',
    },
    taskGroups,
    new Set(['groupA', 'groupB']),
  );

  assert.deepEqual(statuses, {
    __group__groupA: 'skipped',
    __group__groupB: 'running',
  });
});

test('overlap-only groups do not create collapsed nodes or derived statuses', () => {
  const nodes: GraphNode[] = [
    { id: 'shared', deps: [] },
    { id: 'tail', deps: ['shared'] },
  ];

  const taskGroups = {
    groupA: ['shared'],
    groupB: ['shared'],
  };

  assert.deepEqual(
    collapseGraphGroups(nodes, taskGroups, new Set(['groupA', 'groupB'])),
    nodes,
  );

  assert.deepEqual(
    buildGroupStatusEntries(
      nodes,
      { shared: 'failed' },
      taskGroups,
      new Set(['groupA', 'groupB']),
    ),
    {},
  );
});
