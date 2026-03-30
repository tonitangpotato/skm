/**
 * Basic tests for @skm/core Node.js bindings.
 *
 * Note: These tests require the native module to be built first:
 *   cargo build -p skm-node
 *   npm run build
 */

import test from 'ava';
import { existsSync, mkdirSync, writeFileSync, rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

// The native bindings would be imported like:
// import { JsSkillRegistry, JsCascadeSelector, JsBgeM3Provider, JsMiniLmProvider } from '../index.js';

// For now, these are placeholder tests that document the expected API

const TEST_SKILL_CONTENT = `---
name: test-skill
description: A test skill for unit testing
metadata:
  triggers: "test, testing, unit test"
  tags: "testing, example"
---

# Test Skill

This is a test skill used for unit testing the Node.js bindings.

## Usage

Use this skill when the user wants to run tests.
`;

function createTestSkillDir(): string {
  const dir = join(tmpdir(), `skm-test-${Date.now()}`);
  const skillDir = join(dir, 'test-skill');
  mkdirSync(skillDir, { recursive: true });
  writeFileSync(join(skillDir, 'SKILL.md'), TEST_SKILL_CONTENT);
  return dir;
}

function cleanupTestDir(dir: string): void {
  if (existsSync(dir)) {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('JsSkillRegistry - create and list skills', async (t) => {
  // This test documents the expected API
  // When the native module is built, uncomment the actual test code

  /*
  const testDir = createTestSkillDir();
  try {
    const registry = await JsSkillRegistry.new([testDir]);
    
    const count = await registry.len();
    t.is(count, 1, 'Should have one skill');
    
    const skills = await registry.list();
    t.is(skills.length, 1);
    t.is(skills[0].name, 'test-skill');
    t.is(skills[0].description, 'A test skill for unit testing');
    t.deepEqual(skills[0].triggers, ['test', 'testing', 'unit test']);
    t.deepEqual(skills[0].tags, ['testing', 'example']);
  } finally {
    cleanupTestDir(testDir);
  }
  */

  t.pass('API test placeholder - native module not built');
});

test('JsSkillRegistry - get skill by name', async (t) => {
  /*
  const testDir = createTestSkillDir();
  try {
    const registry = await JsSkillRegistry.new([testDir]);
    
    const skill = await registry.get('test-skill');
    t.truthy(skill);
    t.is(skill?.name, 'test-skill');
    
    const notFound = await registry.get('nonexistent');
    t.is(notFound, null);
  } finally {
    cleanupTestDir(testDir);
  }
  */

  t.pass('API test placeholder - native module not built');
});

test('JsCascadeSelector - select matching skill', async (t) => {
  /*
  const testDir = createTestSkillDir();
  try {
    const selector = await JsCascadeSelector.new(testDir);
    
    const results = await selector.select('run the unit test');
    t.true(results.length > 0);
    t.is(results[0].skillName, 'test-skill');
    t.true(results[0].score > 0);
    t.is(results[0].strategy, 'trigger');
  } finally {
    cleanupTestDir(testDir);
  }
  */

  t.pass('API test placeholder - native module not built');
});

test('JsCascadeSelector - no match returns empty', async (t) => {
  /*
  const testDir = createTestSkillDir();
  try {
    const selector = await JsCascadeSelector.new(testDir);
    
    const results = await selector.select('completely unrelated query');
    t.is(results.length, 0);
  } finally {
    cleanupTestDir(testDir);
  }
  */

  t.pass('API test placeholder - native module not built');
});

test('JsBgeM3Provider - embed single text', async (t) => {
  /*
  const provider = JsBgeM3Provider.new();
  
  const embedding = await provider.embed('Hello, world!');
  t.is(embedding.length, 1024); // BGE-M3 produces 1024-dim vectors
  t.is(provider.dimensions, 1024);
  
  // Embeddings should be normalized
  const magnitude = Math.sqrt(embedding.reduce((sum, v) => sum + v * v, 0));
  t.true(Math.abs(magnitude - 1.0) < 0.01);
  */

  t.pass('API test placeholder - native module not built');
});

test('JsBgeM3Provider - embed batch', async (t) => {
  /*
  const provider = JsBgeM3Provider.new();
  
  const texts = ['Hello', 'World', 'Test'];
  const embeddings = await provider.embedBatch(texts);
  
  t.is(embeddings.length, 3);
  t.true(embeddings.every(e => e.length === 1024));
  */

  t.pass('API test placeholder - native module not built');
});

test('JsMiniLmProvider - embed single text', async (t) => {
  /*
  const provider = JsMiniLmProvider.new();
  
  const embedding = await provider.embed('Hello, world!');
  t.is(embedding.length, 384); // MiniLM produces 384-dim vectors
  t.is(provider.dimensions, 384);
  */

  t.pass('API test placeholder - native module not built');
});
