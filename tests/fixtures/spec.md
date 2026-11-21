# Checkout spec

A short fixture for manual and PTY validation.

## Flow

The checkout flow validates the cart, calculates tax, and creates a payment intent.

- Retry failed payments
- Surface a clear error

## Diagram

```mermaid
flowchart LR
  Cart --> Tax --> Payment
```

![red dot](red.png)

| Step | Owner |
| --- | --- |
| Cart | Web |
| Tax | Payments |
