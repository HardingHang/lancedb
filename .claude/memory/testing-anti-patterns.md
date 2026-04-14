---
name: Testing Anti-Patterns Reference
description: Common testing mistakes to avoid including mock testing, test-only methods, and incomplete mocks
type: reference
originSessionId: e8b21aee-a8f9-4a27-9661-d8015fabaf6c
---
**Load this reference when:** writing or changing tests, adding mocks, or tempted to add test-only methods to production code.

**Core principle:** "Test what the code does, not what the mocks do."

**The Iron Laws:**
1. NEVER test mock behavior
2. NEVER add test-only methods to production classes
3. NEVER mock without understanding dependencies

## Anti-Pattern 1: Testing Mock Behavior

**Violation:** Asserting that mock elements exist
**Fix:** Test real component behavior or unmock it

**Gate Function:**
Before asserting on any mock element, ask: "Am I testing real component behavior or just mock existence?"
If testing mock existence: STOP - Delete the assertion or unmock the component

## Anti-Pattern 2: Test-Only Methods in Production

**Violation:** Adding methods like `destroy()` only used in tests
**Fix:** Move to test utilities instead

**Gate Function:**
Before adding any method to production class, ask: "Is this only used by tests?"
If yes: STOP - Put it in test utilities instead

## Anti-Pattern 3: Mocking Without Understanding

**Violation:** Mocking methods whose side effects tests depend on
**Fix:** Mock at correct level - only the slow/external operation

**Gate Function:**
Before mocking any method:
1. Ask: "What side effects does the real method have?"
2. Ask: "Does this test depend on any of those side effects?"
3. Run test with real implementation FIRST
4. THEN add minimal mocking at the right level

## Anti-Pattern 4: Incomplete Mocks

**Violation:** Only mocking fields you think you need
**Fix:** Mirror real API completely with ALL fields

**Iron Rule:** "Mock the COMPLETE data structure as it exists in reality"

## Anti-Pattern 5: Integration Tests as Afterthought

**Violation:** "Implementation complete, ready for testing"
**Fix:** TDD - tests are part of implementation

## Red Flags

- Assertion checks for `*-mock` test IDs
- Methods only called in test files
- Mock setup is >50% of test
- Test fails when you remove mock
- Can't explain why mock is needed
- Mocking "just to be safe"

## Quick Reference

| Anti-Pattern | Fix |
|--------------|-----|
| Assert on mock elements | Test real component or unmock it |
| Test-only methods in production | Move to test utilities |
| Mock without understanding | Understand dependencies first, mock minimally |
| Incomplete mocks | Mirror real API completely |
| Tests as afterthought | TDD - tests first |
| Over-complex mocks | Consider integration tests |

**The Bottom Line:** Mocks are tools to isolate, not things to test.