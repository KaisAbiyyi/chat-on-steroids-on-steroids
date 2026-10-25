import test from "node:test";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {generateFixture} from "./create-fixture.mjs";

test("fixed synthetic content is deterministic, 6,000 distinct messages and 3,000 turn pairs", () => {
  const a = generateFixture();
  const b = generateFixture();
  assert.equal(a.length, 6000);
  assert.deepEqual(a,b);
  assert.equal(new Set(a.map(x=>x.messageId)).size,6000);
  assert.equal(new Set(a.map(x=>x.turnId)).size,3000);
  for (let i=0;i<a.length;i+=2) {
    assert.equal(a[i].kind,"user_message");
    assert.equal(a[i+1].kind,"assistant_message");
    assert.equal(a[i].turnId,a[i+1].turnId);
    assert.match(a[i].text,/^PERF01 public synthetic /);
  }
  const hash = createHash("sha256").update(JSON.stringify(a)).digest("hex");
  assert.equal(hash,"bbe3f0dadd3369fd2574d7212b646bd725633697eec4386a5c4293b8ed71cb6a");
  assert.equal(hash,createHash("sha256").update(JSON.stringify(b)).digest("hex"));
});
test("large-history fixture admission rejects oversize and malformed turn count", () => {
  for (const input of [0,-1,3001,3.5,NaN,"3000",Infinity]) {
    assert.throws(()=>generateFixture(input),/Invalid synthetic turn count/);
  }
});
