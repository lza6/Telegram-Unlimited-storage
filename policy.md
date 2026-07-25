# Policy

## Coverage gate
- Minimum aggregate coverage target: 80% where measurable.
- Coverage is never inferred from test success.

## External-call safety
- paid_api_real_call = false
- No production deployment, push, publish, or destructive data action without explicit approval.

## Review gate
- High-risk nodes require a read-only independent Critic before Done.
