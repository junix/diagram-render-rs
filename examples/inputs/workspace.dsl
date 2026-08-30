workspace "Payments" "Structurizr parser example" {
  !identifiers hierarchical

  model {
    customer = person "Customer"
    payments = softwareSystem "Payments" {
      api = container "API" "Handles payment requests" "Rust"
      database = container "Database" "Stores payments" "PostgreSQL"
      api -> database "Reads from and writes to" "SQL"
    }
    customer -> payments "Uses"
  }

  views {
    systemContext payments "context" {
      include *
      autoLayout lr
    }
    container payments "containers" {
      include *
      autoLayout lr
    }
  }
}
