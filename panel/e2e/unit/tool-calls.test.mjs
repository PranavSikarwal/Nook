import assert from 'node:assert/strict'
import { test } from 'node:test'
import { applyToolEvent, describeToolCall } from '../../src/lib/toolCalls.ts'

const started = (call_id, name, args) => ({
  type: 'tool_call_started',
  id: 'r1',
  chat_id: 'c1',
  message_id: 'm1',
  call_id,
  name,
  arguments: JSON.stringify(args),
})

const finished = (call_id, result) => ({
  type: 'tool_call_finished',
  id: 'r1',
  chat_id: 'c1',
  message_id: 'm1',
  call_id,
  result,
})

test('a started event adds a running call', () => {
  const calls = applyToolEvent([], started('a', 'nook:web_fetch', { url: 'https://x.test/p' }))
  assert.equal(calls.length, 1)
  assert.equal(calls[0].call_id, 'a')
  assert.equal(calls[0].status, 'running')
})

test('a finished event marks the matching call finished and keeps the others', () => {
  let calls = applyToolEvent([], started('a', 'nook:web_fetch', { url: 'https://x.test/p' }))
  calls = applyToolEvent(calls, started('b', 'nook:web_search', { query: 'cats' }))
  calls = applyToolEvent(calls, finished('a', 'page text'))
  assert.deepEqual(calls.map((c) => [c.call_id, c.status]), [['a', 'finished'], ['b', 'running']])
  assert.equal(calls[0].result, 'page text')
})

test('a repeated started event does not duplicate the call', () => {
  const event = started('a', 'nook:web_fetch', { url: 'https://x.test/p' })
  const calls = applyToolEvent(applyToolEvent([], event), event)
  assert.equal(calls.length, 1)
})

test('a finished event for an unknown call is added as finished', () => {
  const calls = applyToolEvent([], finished('z', 'ok'))
  assert.deepEqual(calls.map((c) => [c.call_id, c.status]), [['z', 'finished']])
})

test('events of other types leave the list unchanged', () => {
  const calls = applyToolEvent([], { type: 'text_delta', id: 'r', chat_id: 'c', message_id: 'm', text: 't' })
  assert.deepEqual(calls, [])
})

test('describes a running fetch with its host', () => {
  const [call] = applyToolEvent([], started('a', 'nook:web_fetch', { url: 'https://docs.x.test/p?q=1' }))
  assert.equal(describeToolCall(call, true), 'Running web fetch docs.x.test')
})

test('describes a running search with its query', () => {
  const [call] = applyToolEvent([], started('a', 'nook:web_search', { query: 'cats' }))
  assert.equal(describeToolCall(call, true), 'Running web search cats')
})

test('describes a finished call without the target', () => {
  let calls = applyToolEvent([], started('a', 'nook:web_fetch', { url: 'https://x.test/p' }))
  calls = applyToolEvent(calls, finished('a', 'done'))
  assert.equal(describeToolCall(calls[0], true), 'Finished web fetch')
})

test('describes a call that never finished as stopped once the reply ended', () => {
  const [call] = applyToolEvent([], started('a', 'nook:web_fetch', { url: 'https://x.test/p' }))
  assert.equal(describeToolCall(call, false), 'Stopped web fetch')
})

test('handles unreadable arguments and unknown tool names', () => {
  const [call] = applyToolEvent([], {
    ...started('a', 'acme:do_thing', {}),
    arguments: 'not json',
  })
  assert.equal(describeToolCall(call, true), 'Running do thing')
})

test('a file tool call names its path', () => {
  const [call] = applyToolEvent([], started('a', 'nook:read_local_file', { path: '/work/notes.txt' }))
  assert.equal(describeToolCall(call, true), 'Running read local file /work/notes.txt')
  assert.equal(describeToolCall({ ...call, status: 'finished' }, true), 'Finished read local file')
})

test('a command call shows the command line', () => {
  const [call] = applyToolEvent([], started('a', 'nook:run_command', { argv: ['ls', '-la'], cwd: '/work' }))
  assert.equal(describeToolCall(call, true), 'Running run command ls -la')
})

test('an MCP call names the server and the tool', () => {
  const [call] = applyToolEvent([], started('a', 'mcp:files:read_text', {}))
  assert.equal(describeToolCall(call, true), 'Running files read text')
})
