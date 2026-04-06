pub mod employee_service;
pub use employee_service::EmployeeService;

#[cfg(test)]
mod tests {
    use crate::models::department::*;
    use crate::models::employee::*;
    use crate::repository::department_repo::DepartmentRepo;
    use crate::repository::employee_repo::EmployeeRepo;
    use saas_common::pagination::PaginationParams;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_departments.sql"),
            include_str!("../../migrations/002_create_employees.sql"),
        ];
        let migration_names = [
            "001_create_departments.sql",
            "002_create_employees.sql",
        ];
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (filename TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (i, sql) in sql_files.iter().enumerate() {
            let name = migration_names[i];
            let already_applied: bool =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _migrations WHERE filename = ?")
                    .bind(name)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    > 0;
            if !already_applied {
                let mut tx = pool.begin().await.unwrap();
                sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query("INSERT INTO _migrations (filename, applied_at) VALUES (?, ?)")
                    .bind(name)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }
        }
        pool
    }

    async fn setup_repos() -> (DepartmentRepo, EmployeeRepo) {
        let pool = setup().await;
        (
            DepartmentRepo::new(pool.clone()),
            EmployeeRepo::new(pool),
        )
    }

    #[tokio::test]
    async fn test_department_crud() {
        let (dept_repo, _) = setup_repos().await;

        // Create
        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Engineering".into(),
                parent_id: None,
                manager_id: None,
                cost_center: Some("CC-001".into()),
            })
            .await
            .unwrap();
        assert_eq!(dept.name, "Engineering");
        assert_eq!(dept.cost_center, Some("CC-001".into()));

        // Read
        let fetched = dept_repo.get_by_id(&dept.id).await.unwrap();
        assert_eq!(fetched.id, dept.id);

        // List
        let depts = dept_repo.list().await.unwrap();
        assert_eq!(depts.len(), 1);

        // Update
        let updated = dept_repo
            .update(
                &dept.id,
                Some("Engineering & QA"),
                None,
                None,
                Some("CC-002"),
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "Engineering & QA");
        assert_eq!(updated.cost_center, Some("CC-002".into()));
    }

    #[tokio::test]
    async fn test_employee_crud() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Sales".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        // Create
        let emp = emp_repo
            .create(&CreateEmployee {
                first_name: "Alice".into(),
                last_name: "Smith".into(),
                email: "alice@example.com".into(),
                phone: Some("555-0101".into()),
                hire_date: "2025-01-15".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "Sales Rep".into(),
                employee_number: "EMP-001".into(),
            })
            .await
            .unwrap();
        assert_eq!(emp.first_name, "Alice");
        assert_eq!(emp.status, "active");
        assert_eq!(emp.department_id, dept.id);

        // Read
        let fetched = emp_repo.get_by_id(&emp.id).await.unwrap();
        assert_eq!(fetched.email, "alice@example.com");

        // Update
        let updated = emp_repo
            .update(
                &emp.id,
                Some("Alice"),
                Some("Johnson"),
                None,
                None,
                None,
                None,
                Some("Senior Sales Rep"),
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.last_name, "Johnson");
        assert_eq!(updated.job_title, "Senior Sales Rep");
    }

    #[tokio::test]
    async fn test_employee_termination() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "HR".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        let emp = emp_repo
            .create(&CreateEmployee {
                first_name: "Bob".into(),
                last_name: "Brown".into(),
                email: "bob@example.com".into(),
                phone: None,
                hire_date: "2024-06-01".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "HR Specialist".into(),
                employee_number: "EMP-002".into(),
            })
            .await
            .unwrap();

        assert_eq!(emp.status, "active");
        assert!(emp.termination_date.is_none());

        // Terminate
        emp_repo.terminate(&emp.id, "2025-06-30").await.unwrap();
        let terminated = emp_repo.get_by_id(&emp.id).await.unwrap();
        assert_eq!(terminated.status, "terminated");
        assert_eq!(terminated.termination_date, Some("2025-06-30".into()));
    }

    #[tokio::test]
    async fn test_org_chart() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Engineering".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        // Create manager
        let manager = emp_repo
            .create(&CreateEmployee {
                first_name: "Carol".into(),
                last_name: "Davis".into(),
                email: "carol@example.com".into(),
                phone: None,
                hire_date: "2023-01-01".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "VP Engineering".into(),
                employee_number: "EMP-010".into(),
            })
            .await
            .unwrap();

        // Create reports
        emp_repo
            .create(&CreateEmployee {
                first_name: "Dave".into(),
                last_name: "Wilson".into(),
                email: "dave@example.com".into(),
                phone: None,
                hire_date: "2024-01-01".into(),
                department_id: dept.id.clone(),
                reports_to: Some(manager.id.clone()),
                job_title: "Software Engineer".into(),
                employee_number: "EMP-011".into(),
            })
            .await
            .unwrap();

        emp_repo
            .create(&CreateEmployee {
                first_name: "Eve".into(),
                last_name: "Martinez".into(),
                email: "eve@example.com".into(),
                phone: None,
                hire_date: "2024-02-01".into(),
                department_id: dept.id.clone(),
                reports_to: Some(manager.id.clone()),
                job_title: "QA Engineer".into(),
                employee_number: "EMP-012".into(),
            })
            .await
            .unwrap();

        let org_chart = emp_repo.get_org_chart().await.unwrap();
        assert_eq!(org_chart.len(), 3);

        // Verify department name is populated
        for node in &org_chart {
            assert_eq!(node.department_name, Some("Engineering".into()));
        }
    }

    #[tokio::test]
    async fn test_direct_reports() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Marketing".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        let manager = emp_repo
            .create(&CreateEmployee {
                first_name: "Frank".into(),
                last_name: "Lee".into(),
                email: "frank@example.com".into(),
                phone: None,
                hire_date: "2023-06-01".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "Marketing Director".into(),
                employee_number: "EMP-020".into(),
            })
            .await
            .unwrap();

        emp_repo
            .create(&CreateEmployee {
                first_name: "Grace".into(),
                last_name: "Kim".into(),
                email: "grace@example.com".into(),
                phone: None,
                hire_date: "2024-01-01".into(),
                department_id: dept.id.clone(),
                reports_to: Some(manager.id.clone()),
                job_title: "Content Writer".into(),
                employee_number: "EMP-021".into(),
            })
            .await
            .unwrap();

        emp_repo
            .create(&CreateEmployee {
                first_name: "Hank".into(),
                last_name: "Zhao".into(),
                email: "hank@example.com".into(),
                phone: None,
                hire_date: "2024-03-01".into(),
                department_id: dept.id.clone(),
                reports_to: Some(manager.id.clone()),
                job_title: "Designer".into(),
                employee_number: "EMP-022".into(),
            })
            .await
            .unwrap();

        let reports = emp_repo.get_direct_reports(&manager.id).await.unwrap();
        assert_eq!(reports.len(), 2);
    }

    #[tokio::test]
    async fn test_employee_filtering_by_department() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let eng_dept = dept_repo
            .create(&CreateDepartment {
                name: "Engineering".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        let sales_dept = dept_repo
            .create(&CreateDepartment {
                name: "Sales".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        // Create employees in different departments
        emp_repo
            .create(&CreateEmployee {
                first_name: "Ian".into(),
                last_name: "Cooper".into(),
                email: "ian@example.com".into(),
                phone: None,
                hire_date: "2025-01-01".into(),
                department_id: eng_dept.id.clone(),
                reports_to: None,
                job_title: "DevOps".into(),
                employee_number: "EMP-030".into(),
            })
            .await
            .unwrap();

        emp_repo
            .create(&CreateEmployee {
                first_name: "Jane".into(),
                last_name: "Taylor".into(),
                email: "jane@example.com".into(),
                phone: None,
                hire_date: "2025-01-15".into(),
                department_id: sales_dept.id.clone(),
                reports_to: None,
                job_title: "Account Exec".into(),
                employee_number: "EMP-031".into(),
            })
            .await
            .unwrap();

        let pag = PaginationParams { page: Some(1), per_page: Some(10) };

        // Filter by Engineering
        let (eng_emps, eng_total) = emp_repo
            .list(
                &pag,
                &EmployeeFilters {
                    department_id: Some(eng_dept.id.clone()),
                    status: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(eng_total, 1);
        assert_eq!(eng_emps.len(), 1);
        assert_eq!(eng_emps[0].first_name, "Ian");

        // Filter by Sales
        let (sales_emps, sales_total) = emp_repo
            .list(
                &pag,
                &EmployeeFilters {
                    department_id: Some(sales_dept.id.clone()),
                    status: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(sales_total, 1);
        assert_eq!(sales_emps[0].first_name, "Jane");

        // No filter
        let (all_emps, all_total) = emp_repo
            .list(
                &pag,
                &EmployeeFilters {
                    department_id: None,
                    status: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(all_total, 2);
        assert_eq!(all_emps.len(), 2);
    }

    #[tokio::test]
    async fn test_employee_pagination() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Support".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        // Create 5 employees
        for i in 0..5 {
            emp_repo
                .create(&CreateEmployee {
                    first_name: format!("User{}", i),
                    last_name: "Test".into(),
                    email: format!("user{}@example.com", i),
                    phone: None,
                    hire_date: "2025-01-01".into(),
                    department_id: dept.id.clone(),
                    reports_to: None,
                    job_title: "Support Agent".into(),
                    employee_number: format!("EMP-{:03}", 40 + i),
                })
                .await
                .unwrap();
        }

        let filters = EmployeeFilters {
            department_id: None,
            status: None,
        };

        // Page 1 with per_page=2
        let pag1 = PaginationParams { page: Some(1), per_page: Some(2) };
        let (page1, total) = emp_repo.list(&pag1, &filters).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 2);

        // Page 2
        let pag2 = PaginationParams { page: Some(2), per_page: Some(2) };
        let (page2, _) = emp_repo.list(&pag2, &filters).await.unwrap();
        assert_eq!(page2.len(), 2);

        // Page 3
        let pag3 = PaginationParams { page: Some(3), per_page: Some(2) };
        let (page3, _) = emp_repo.list(&pag3, &filters).await.unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[tokio::test]
    async fn test_department_hierarchy() {
        let (dept_repo, _) = setup_repos().await;

        let parent = dept_repo
            .create(&CreateDepartment {
                name: "Technology".into(),
                parent_id: None,
                manager_id: None,
                cost_center: Some("CC-TECH".into()),
            })
            .await
            .unwrap();

        let child = dept_repo
            .create(&CreateDepartment {
                name: "Backend".into(),
                parent_id: Some(parent.id.clone()),
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        assert_eq!(child.parent_id, Some(parent.id));
        assert_eq!(child.name, "Backend");

        let depts = dept_repo.list().await.unwrap();
        assert_eq!(depts.len(), 2);
    }

    #[tokio::test]
    async fn test_employee_filter_by_status() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Operations".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        let emp = emp_repo
            .create(&CreateEmployee {
                first_name: "Kate".into(),
                last_name: "Nguyen".into(),
                email: "kate@example.com".into(),
                phone: None,
                hire_date: "2024-01-01".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "Operations Lead".into(),
                employee_number: "EMP-050".into(),
            })
            .await
            .unwrap();

        // Terminate one employee
        emp_repo.terminate(&emp.id, "2025-06-01").await.unwrap();

        let pag = PaginationParams { page: Some(1), per_page: Some(10) };

        // Filter by active
        let (active, active_total) = emp_repo
            .list(
                &pag,
                &EmployeeFilters {
                    department_id: None,
                    status: Some("active".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(active_total, 0);

        // Filter by terminated
        let (terminated, term_total) = emp_repo
            .list(
                &pag,
                &EmployeeFilters {
                    department_id: None,
                    status: Some("terminated".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(term_total, 1);
        assert_eq!(terminated[0].first_name, "Kate");
    }

    #[tokio::test]
    async fn test_auto_create_employee_from_hire_event() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Engineering".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        // Simulate the hiring event by creating an employee using the same
        // logic that handle_application_hired uses (repo-level test)
        let app_id = uuid::Uuid::new_v4().to_string();
        let employee_number = format!("EMP-AUTO-{}", &app_id[..8.min(app_id.len())]);
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let input = CreateEmployee {
            first_name: "Jane".into(),
            last_name: "Doe".into(),
            email: "jane.doe@example.com".into(),
            phone: None,
            hire_date: today,
            department_id: dept.id.clone(),
            reports_to: None,
            job_title: "Software Engineer".into(),
            employee_number: employee_number.clone(),
        };

        let emp = emp_repo.create(&input).await.unwrap();

        assert_eq!(emp.first_name, "Jane");
        assert_eq!(emp.last_name, "Doe");
        assert_eq!(emp.email, "jane.doe@example.com");
        assert_eq!(emp.job_title, "Software Engineer");
        assert_eq!(emp.department_id, dept.id);
        assert_eq!(emp.status, "active");
        assert!(emp.employee_number.starts_with("EMP-AUTO-"));
    }

    #[tokio::test]
    async fn test_auto_create_employee_number_format() {
        // Verify the employee number generation logic
        let app_id = "abcdef12-3456-7890-abcd-ef1234567890";
        let emp_num = format!("EMP-AUTO-{}", &app_id[..8.min(app_id.len())]);
        assert_eq!(emp_num, "EMP-AUTO-abcdef12");

        // Short application ID
        let short_id = "abc";
        let emp_num_short = format!("EMP-AUTO-{}", &short_id[..8.min(short_id.len())]);
        assert_eq!(emp_num_short, "EMP-AUTO-abc");

        // Exactly 8 chars
        let exact_id = "12345678";
        let emp_num_exact = format!("EMP-AUTO-{}", &exact_id[..8.min(exact_id.len())]);
        assert_eq!(emp_num_exact, "EMP-AUTO-12345678");
    }

    #[tokio::test]
    async fn test_auto_created_employee_appears_in_list() {
        let (dept_repo, emp_repo) = setup_repos().await;

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Product".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        // Auto-create employee (simulating hire event handler)
        let app_id = uuid::Uuid::new_v4().to_string();
        let employee_number = format!("EMP-AUTO-{}", &app_id[..8.min(app_id.len())]);

        emp_repo
            .create(&CreateEmployee {
                first_name: "Auto".into(),
                last_name: "Hire".into(),
                email: "auto.hire@example.com".into(),
                phone: None,
                hire_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "Product Manager".into(),
                employee_number: employee_number,
            })
            .await
            .unwrap();

        // Verify it appears in the employee list
        let pag = PaginationParams { page: Some(1), per_page: Some(10) };
        let (emps, total) = emp_repo
            .list(
                &pag,
                &EmployeeFilters {
                    department_id: Some(dept.id),
                    status: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(total, 1);
        assert_eq!(emps[0].first_name, "Auto");
        assert_eq!(emps[0].last_name, "Hire");
    }

    #[tokio::test]
    async fn test_employee_update_returns_updated_fields() {
        let pool = setup().await;
        let emp_repo = EmployeeRepo::new(pool.clone());
        let dept_repo = DepartmentRepo::new(pool);

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Test Dept".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        let emp = emp_repo
            .create(&CreateEmployee {
                first_name: "John".into(),
                last_name: "Doe".into(),
                email: "john@example.com".into(),
                phone: None,
                hire_date: "2025-01-01".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "Engineer".into(),
                employee_number: "EMP-001".into(),
            })
            .await
            .unwrap();

        let updated = emp_repo
            .update(
                &emp.id,
                Some("Jane"),
                None,
                Some("jane@example.com"),
                None,
                None,
                None,
                Some("Senior Engineer"),
                None,
            )
            .await
            .unwrap();

        assert_eq!(updated.first_name, "Jane");
        assert_eq!(updated.email, "jane@example.com");
        assert_eq!(updated.job_title, "Senior Engineer");
        // Last name unchanged
        assert_eq!(updated.last_name, "Doe");
    }

    #[tokio::test]
    async fn test_employee_terminate_sets_status() {
        let pool = setup().await;
        let emp_repo = EmployeeRepo::new(pool.clone());
        let dept_repo = DepartmentRepo::new(pool);

        let dept = dept_repo
            .create(&CreateDepartment {
                name: "Finance".into(),
                parent_id: None,
                manager_id: None,
                cost_center: None,
            })
            .await
            .unwrap();

        let emp = emp_repo
            .create(&CreateEmployee {
                first_name: "Alice".into(),
                last_name: "Smith".into(),
                email: "alice@example.com".into(),
                phone: None,
                hire_date: "2025-01-01".into(),
                department_id: dept.id.clone(),
                reports_to: None,
                job_title: "Analyst".into(),
                employee_number: "EMP-002".into(),
            })
            .await
            .unwrap();

        assert_eq!(emp.status, "active");

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        emp_repo.terminate(&emp.id, &today).await.unwrap();

        let terminated = emp_repo.get_by_id(&emp.id).await.unwrap();
        assert_eq!(terminated.status, "terminated");
    }
}
