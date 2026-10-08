// The Black Friday deck, as the agent wrote it: deck.json and g1–g3, with
// the files its refs point at. Generated from ~/.deck/decks/d-1791206273-4084.deck.
window.DECK = {
 "id": "d-1791206273-4084",
 "title": "Checkout is slow: take the receipt off the request?",
 "groups": [
  {
   "say": "Checkout feels slow, and last month's traces say why. Here is everything [checkout] does before the customer sees their order:\n\n1. [point 19-21 today] Charge the card and create the order. A few hundred milliseconds.\n\n2. [point 23 slow] Render the receipt PDF in headless Chrome. Anywhere from 0.4 to 3.8 seconds.\n\n3. [point 24] Email it, and only then answer.\n\n[pause] In [plan], that is a **4.2 second p99**, all of it spent watching a spinner for an email nobody reads at checkout.",
   "refs": [
    {
     "kind": "code",
     "name": "checkout",
     "note": "POST /checkout, today",
     "file": "apps/checkout/src/orders/checkout.service.ts",
     "range": [
      18,
      27
     ],
     "src": [
      "import { Injectable } from \"@nestjs/common\";",
      "",
      "import { Payments } from \"@/integrations/payments\";",
      "import { OrdersRepository } from \"./orders.repository\";",
      "import { InvoiceRenderer } from \"../invoices/invoice.renderer\";",
      "import { Mailer } from \"@/integrations/mailer\";",
      "import type { Cart, PlacedOrder } from \"./order.types\";",
      "",
      "@Injectable()",
      "export class CheckoutService {",
      "  constructor(",
      "    private readonly payments: Payments,",
      "    private readonly orders: OrdersRepository,",
      "    private readonly invoices: InvoiceRenderer,",
      "    private readonly mailer: Mailer,",
      "  ) {}",
      "",
      "  /** POST /checkout — the customer is watching a spinner until this returns. */",
      "  async placeOrder(cart: Cart): Promise<PlacedOrder> {",
      "    const charge = await this.payments.charge(cart.total, cart.paymentMethod);",
      "    const order = await this.orders.create(cart, charge.id);",
      "",
      "    const pdf = await this.invoices.render(order); // headless Chrome, 0.4–3.8 s",
      "    await this.mailer.send(order.email, \"Your receipt\", { attachments: [pdf] });",
      "",
      "    return { orderId: order.id, total: order.total };",
      "  }",
      "}"
     ]
    },
    {
     "kind": "page",
     "name": "plan",
     "note": "what each option does in production"
    }
   ]
  },
  {
   "say": "The plan: **answer first, send the receipt after.**\n\n1. [point +23-24 offload] [checkout] puts one job on a queue instead of rendering. The p99 drops to 260 milliseconds.\n\n2. [pause] But the work does not disappear. It moves to a worker, and the worker has its own limits.\n\n3. [point spike] On Black Friday, orders rise twelve-fold by noon. [point backlog] Four fixed workers fall behind, and receipts arrive **nine minutes late**.\n\nThat backlog is the real cost of this plan, and it is the part worth deciding on.",
   "refs": [
    {
     "kind": "code",
     "name": "checkout",
     "note": "answer first, render later",
     "file": "apps/checkout/src/orders/checkout.service.ts",
     "range": [
      18,
      27
     ],
     "src": [
      "import { Injectable } from \"@nestjs/common\";",
      "",
      "import { Payments } from \"@/integrations/payments\";",
      "import { OrdersRepository } from \"./orders.repository\";",
      "import { InvoiceRenderer } from \"../invoices/invoice.renderer\";",
      "import { Mailer } from \"@/integrations/mailer\";",
      "import type { Cart, PlacedOrder } from \"./order.types\";",
      "",
      "@Injectable()",
      "export class CheckoutService {",
      "  constructor(",
      "    private readonly payments: Payments,",
      "    private readonly orders: OrdersRepository,",
      "    private readonly invoices: InvoiceRenderer,",
      "    private readonly mailer: Mailer,",
      "  ) {}",
      "",
      "  /** POST /checkout — the customer is watching a spinner until this returns. */",
      "  async placeOrder(cart: Cart): Promise<PlacedOrder> {",
      "    const charge = await this.payments.charge(cart.total, cart.paymentMethod);",
      "    const order = await this.orders.create(cart, charge.id);",
      "",
      "    const pdf = await this.invoices.render(order); // headless Chrome, 0.4–3.8 s",
      "    await this.mailer.send(order.email, \"Your receipt\", { attachments: [pdf] });",
      "",
      "    return { orderId: order.id, total: order.total };",
      "  }",
      "}"
     ],
     "after": [
      "  /** POST /checkout — the customer is watching a spinner until this returns. */",
      "  async placeOrder(cart: Cart): Promise<PlacedOrder> {",
      "    const charge = await this.payments.charge(cart.total, cart.paymentMethod);",
      "    const order = await this.orders.create(cart, charge.id);",
      "",
      "    // The receipt is rendered and emailed by InvoiceWorker, after we answer.",
      "    await this.invoiceQueue.add(\"render\", { orderId: order.id });",
      "",
      "    return { orderId: order.id, total: order.total };",
      "  }"
     ]
    },
    {
     "kind": "page",
     "name": "plan",
     "note": "what each option does in production"
    }
   ]
  },
  {
   "say": "[point 17-21] [worker] does the same render and send, off the request, four at a time.\n\n1. [point 9 autoscale] Scale it on queue depth, from two to twenty workers, and the Black Friday backlog in [plan] stays **under forty seconds**.\n\n2. The rest of the year it idles at two, so it costs less than the checkout machines we would otherwise grow.\n\n[pause] The decision: ship this with autoscaling, or keep the receipt inline and buy bigger checkout machines. I would ship it. **Which way?**",
   "refs": [
    {
     "kind": "code",
     "name": "worker",
     "note": "the receipt, after the order",
     "file": "apps/checkout/src/invoices/invoice.worker.ts",
     "range": [
      8,
      22
     ],
     "src": [
      "import { Processor, WorkerHost } from \"@nestjs/bullmq\";",
      "import type { Job } from \"bullmq\";",
      "",
      "import { InvoiceRenderer } from \"./invoice.renderer\";",
      "import { Mailer } from \"@/integrations/mailer\";",
      "import { OrdersRepository } from \"../orders/orders.repository\";",
      "",
      "/** Renders and emails the receipt, after the customer already has their order. */",
      "@Processor(\"invoices\", { concurrency: 4 })",
      "export class InvoiceWorker extends WorkerHost {",
      "  constructor(",
      "    private readonly invoices: InvoiceRenderer,",
      "    private readonly mailer: Mailer,",
      "    private readonly orders: OrdersRepository,",
      "  ) { super(); }",
      "",
      "  async process(job: Job<{ orderId: string }>) {",
      "    const order = await this.orders.get(job.data.orderId);",
      "    const pdf = await this.invoices.render(order);",
      "    await this.mailer.send(order.email, \"Your receipt\", { attachments: [pdf] });",
      "  }",
      "}"
     ],
     "before": ""
    },
    {
     "kind": "page",
     "name": "plan",
     "note": "what each option does in production"
    }
   ]
  }
 ],
 "page": "play/plan.html"
}
