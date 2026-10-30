// The deck /play/ plays: what the agent wrote with deck new, deck group and deck seal, and the files
// its refs point at. The app, its code and its test runs are made up; the answers are recorded.
// js/play.js reads it the way deck reads a deck: the say's [point] and [name] marks, the refs, the
// --after change, the --values, and the --ask pills.
window.PLAY_DECK = {
  title: 'Fix random logouts',
  files: {
    'src/auth/session.ts': [
      'import { db } from "../db";',
      'import { sign, AuthError } from "./tokens";',
      '',
      'export const ACCESS_TTL = 15 * 60;   // 15 minutes',
      'export const SESSION_TTL = 60 * 60;  // 1 hour',
      '',
      'export async function refresh(token: string) {',
      '  const session = await db.sessions.find(token);',
      '  if (!session || session.expiresAt < Date.now()) {',
      '    throw new AuthError("session expired");',
      '  }',
      '  session.expiresAt = Date.now() + SESSION_TTL * 1000;',
      '  await db.sessions.save(session);',
      '  return sign(session.userId, ACCESS_TTL);',
      '}',
    ],
    'src/auth/logout.ts': [
      'export async function logout(req, res) {',
      '  const token = req.cookies.refresh;',
      '  res.clearCookie("sid");',
      '  res.clearCookie("refresh");',
      '  res.status(204).end();',
      '}',
    ],
  },
  groups: [
    {
      title: "The refresh now extends the session's lifetime",
      confidence: 'high',
      say: 'Users who leave a tab for an hour come back signed out. [point 9-11 refresh] `refresh()` turns away a session past its `expiresAt`, and [point 5 ttl] `SESSION_TTL` gives every session one hour.\n\n[point +5 ttl] So I made it thirty days. [point 12 refresh] Each refresh pushes the session out by the new TTL, so a tab left open over a weekend stays signed in.',
      refs: [
        { kind: 'code', name: 'refresh', note: 'where a session is kept alive', file: 'src/auth/session.ts', range: [7, 15] },
        { kind: 'code', name: 'ttl', note: 'a proposed change', file: 'src/auth/session.ts', range: [5, 5], after: ['export const SESSION_TTL = 30 * 24 * 60 * 60;  // 30 days'] },
      ],
      ask: ['Why thirty days?', 'What else reads SESSION_TTL?'],
    },
    {
      title: 'A refresh is what keeps you signed in',
      confidence: 'high',
      say: 'Every fifteen minutes the access token runs out, and the app calls [point 7-8 refresh] `refresh()` with the refresh token.\n\n[point 9-10 before flow] Before, a session idle for an hour failed this check, and you were signed out. [point 12-14 now flow] Now it passes, and you get a new access token.',
      refs: [
        { kind: 'code', name: 'refresh', note: 'the check, and the new token', file: 'src/auth/session.ts', range: [7, 15] },
        { kind: 'flow', name: 'flow', note: 'a token running out, before and now' },
      ],
      ask: ['Why not a longer access token?', 'What else reads SESSION_TTL?'],
    },
    {
      title: 'Now only logout signs you out',
      confidence: 'medium',
      say: '[point before life] An hour idle used to end a session. [point now life] Now a session lasts thirty days, [point logout life] and logging out is what ends it.',
      refs: [
        { kind: 'page', name: 'life', note: "a session's life, before and now" },
      ],
      ask: ['Why thirty days?', 'Is thirty days safe?'],
    },
    {
      title: 'Tested: a refresh lands thirty days out',
      confidence: 'high',
      say: 'From the test run: [point 12 refresh] `refresh()` set `expiresAt` to 28 November, thirty days on, [point 14 refresh] and handed back a fifteen-minute access token.\n\n[point 8 refresh] The session carries a `revoked` flag, and nothing here reads it.',
      refs: [
        {
          kind: 'code', name: 'refresh', note: "values from the agent's test run · session.spec.ts", file: 'src/auth/session.ts', range: [7, 15],
          values: [
            { line: 8, at: 'session', is: 'Session { userId, expiresAt, revoked }', keys: [
              { key: 'userId', is: '"u_81"' },
              { key: 'expiresAt', is: '2026-10-29T10:14:07Z' },
              { key: 'revoked', is: 'false', why: 'read nowhere' },
            ] },
            { line: 12, at: 'expiresAt', is: '2026-11-28T10:14:07Z', lit: true },
            { line: 14, at: 'ACCESS_TTL', is: '900' },
          ],
        },
      ],
      ask: ['Does logout still kill the session?', 'Why thirty days?', 'What else reads SESSION_TTL?'],
    },
  ],
  // What the agent says back, recorded. `brings` is what it puts on screen with deck bring and deck values.
  answers: {
    'Does logout still kill the session?': {
      a: 'Logout clears the cookie but never revokes the refresh token, so with a 30-day TTL a stolen token lives 30 days.',
      at: 'logout.ts:2-4',
      brings: [
        {
          kind: 'code', name: 'logout', note: 'values from logout.spec.ts · after logout', file: 'src/auth/logout.ts', range: [2, 4], brought: true,
          values: [
            { line: 2, at: 'token', is: '"rt_9f2c41"', lit: true },
            { line: 4, at: 'cookie', is: 'cleared' },
            { line: 5, at: 'response', is: '204' },
          ],
        },
        { kind: 'page', name: 'stolen', note: 'a copied refresh token, after logout', brought: true },
      ],
    },
    'Why thirty days?': { a: 'Nothing in the repo asks for it. It is my pick, the usual length for staying signed in, so treat it as a choice and not a requirement.', at: 'session.ts:5' },
    'What else reads SESSION_TTL?': { a: 'Only refresh(), on line 12. The cookie in login.ts sets its own lifetime and is not changed here.', at: 'session.ts:12' },
    'Why not a longer access token?': { a: 'An access token is checked without a database read, so it cannot be taken back. Keeping it at fifteen minutes keeps that window small.', at: 'session.ts:4' },
    'Is thirty days safe?': { a: 'Only if signing out really ends a session. I have not checked that here: ask me about logout on the next group.', at: 'group 3' },
  },
  unknown: 'This deck is a recording, with its answers written in advance, so I cannot answer that one here. In deck, your own agent answers what you ask, in this thread, and can bring the file that shows it.',
}
