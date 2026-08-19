use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegalDomain {
    Pipa,             // Personal Information Protection Act (개인정보 보호법)
    NetworkAct,       // Act on Promotion of Info & Communications Network (정보통신망법)
    CreditInfoAct,    // Use and Protection of Credit Information Act (신용정보법)
    ECommerceAct,     // Consumer Protection in Electronic Commerce Act (전자상거래법)
    Efta,             // Electronic Financial Transactions Act (전자금융거래법)
    TelecomSecretAct, // Protection of Communications Secrets Act (통신비밀보호법)
    AiBasicAct,       // Framework Act on Artificial Intelligence (인공지능 기본법)
    Hipaa,            // Health Insurance Portability and Accountability Act (45 CFR)
    Gdpr,             // General Data Protection Regulation (EU 2016/679)
    PciDss,           // Payment Card Industry Data Security Standard (v4.0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegulatoryRule {
    pub rule_id: &'static str,
    pub domain: LegalDomain,
    pub statutory_reference: &'static str, // e.g. "개보법 §24의2", "전상법 §21의2①3호", "신정법 §33①"
    pub title: &'static str,
    pub requirement_spec: &'static str,
    pub regex_pattern: Option<&'static str>,
    pub severity: &'static str, // "CRITICAL", "HIGH", "MEDIUM"
}

pub static REGULATORY_SPECS: &[RegulatoryRule] = &[
    // -------------------------------------------------------------
    // 1. PIPA (개인정보 보호법)
    // -------------------------------------------------------------
    RegulatoryRule {
        rule_id: "PIPA_RRN_BAN",
        domain: LegalDomain::Pipa,
        statutory_reference: "개보법 §24의2, 영 §21의2 (주민등록번호 처리의 제한 및 암호화)",
        title: "Resident Registration Number (RRN) Strict Prohibition",
        requirement_spec: "Processing RRNs is strictly prohibited unless specifically mandated by statute. Use CI/DI tokenization instead.",
        regex_pattern: Some(r"\b\d{6}-[1-4]\d{6}\b"),
        severity: "CRITICAL",
    },
    RegulatoryRule {
        rule_id: "PIPA_UNMASKED_PHONE",
        domain: LegalDomain::Pipa,
        statutory_reference: "개보법 §29, 영 §30① (안전조치의무)",
        title: "Unmasked Mobile Phone Number in Source/Logs",
        requirement_spec: "Customer phone numbers must be masked in logs, telemetry, and client-facing outputs.",
        regex_pattern: Some(r"\b01[016789]-\d{3,4}-\d{4}\b"),
        severity: "HIGH",
    },
    RegulatoryRule {
        rule_id: "PIPA_UNMASKED_LOGGING",
        domain: LegalDomain::Pipa,
        statutory_reference: "개보법 §29, 영 §30①5호 (접속기록 보관 및 위변조 방지)",
        title: "Plaintext PII Logged to Telemetry Stream",
        requirement_spec: "Sensitive credentials, RRNs, passwords, and tokens must never be passed to logger macros.",
        regex_pattern: Some(r#"(?i)log::(?:info|debug|error|warn)!\(.*?(?:ssn|rrn|password|resident_number|secret_key).*?\)"#),
        severity: "CRITICAL",
    },
    RegulatoryRule {
        rule_id: "PIPA_INSECURE_HASH",
        domain: LegalDomain::Pipa,
        statutory_reference: "개보법 영 §30①4호 가목 (비밀번호 일방향 암호화)",
        title: "Insecure Password Hashing (MD5/SHA1/Single SHA256)",
        requirement_spec: "Passwords must be hashed using one-way adaptive algorithms (bcrypt, Argon2, scrypt with salt).",
        regex_pattern: Some(r#"(?i)(?:md5|sha1)::digest|crypto::createHash\(['"](?:md5|sha1)['"]\)"#),
        severity: "CRITICAL",
    },

    // -------------------------------------------------------------
    // 2. CREDIT INFORMATION ACT (신용정보법)
    // -------------------------------------------------------------
    RegulatoryRule {
        rule_id: "CREDIT_MARKETING_MISUSE",
        domain: LegalDomain::CreditInfoAct,
        statutory_reference: "신정법 §33①3호, §45의3① (개인신용정보의 이용제한)",
        title: "Payment / Delinquency Data Used for Marketing",
        requirement_spec: "Commercial entities may not use transaction, billing, or delinquency records for marketing or sales recommendations without affirmative separate consent.",
        regex_pattern: Some(r#"(?i)(?:campaign|marketing|promo).*?(?:delinquency|billing_history|overdue_amount|credit_tier)"#),
        severity: "CRITICAL",
    },

    // -------------------------------------------------------------
    // 3. E-COMMERCE ACT & ANTI-DARK-PATTERN (전자상거래법)
    // -------------------------------------------------------------
    RegulatoryRule {
        rule_id: "ECOM_DARK_PATTERN_PRECHECK",
        domain: LegalDomain::ECommerceAct,
        statutory_reference: "전상법 §21의2①2호 (다크패턴 - 사전 선택된 동의 금지)",
        title: "Pre-Ticked Opt-in or Add-on Checkbox (Dark Pattern)",
        requirement_spec: "Pre-ticking opt-in checkboxes, additional products, or marketing consents is illegal under the E-Commerce Act.",
        regex_pattern: Some(r#"(?i)(?:checked|defaultChecked)\s*=\s*(?:true|\{true\})[\s\S]*?(?:marketing|terms_optional|addon|subscribe_promo)"#),
        severity: "CRITICAL",
    },
    RegulatoryRule {
        rule_id: "ECOM_SUBSCRIPTION_PRICE_CHANGE",
        domain: LegalDomain::ECommerceAct,
        statutory_reference: "전상법 §13⑥ (정기결제 증액 및 유료전환 사전고지·동의)",
        title: "Un-consented Subscription Price Increase or Trial Conversion",
        requirement_spec: "Auto-renewals that increase prices or convert free trials to paid must obtain explicit prior affirmative consent.",
        regex_pattern: Some(r#"(?i)(?:auto_bill_price_hike|convert_to_paid_without_notice)"#),
        severity: "CRITICAL",
    },

    // -------------------------------------------------------------
    // 4. EFTA (전자금융거래법)
    // -------------------------------------------------------------
    RegulatoryRule {
        rule_id: "EFTA_ESCROW_SEGREGATION",
        domain: LegalDomain::Efta,
        statutory_reference: "전금법 §25의2, §25의4 (선불충전금 및 정산대금의 별도관리)",
        title: "Commagling Customer Stored-Value or Settlement Escrow Funds",
        requirement_spec: "Customer stored-value funds and merchant settlements must be segregated in external bank trusts/guarantee insurance.",
        regex_pattern: Some(r#"(?i)(?:pool_operating_and_custody_funds|direct_debit_escrow_to_operating)"#),
        severity: "CRITICAL",
    },

    // -------------------------------------------------------------
    // 5. COMMUNICATIONS SECRETS ACT (통신비밀보호법)
    // -------------------------------------------------------------
    RegulatoryRule {
        rule_id: "TELECOM_LOG_PURGE_EARLY",
        domain: LegalDomain::TelecomSecretAct,
        statutory_reference: "통비법 영 §41②2호 (통신사실확인자료 3개월 보관의무)",
        title: "Premature Purge of Telecommunications Access Logs (< 3 Months)",
        requirement_spec: "Internet access, login timestamps, and IP trace records must be retained for at least 3 months for legal compliance.",
        regex_pattern: Some(r#"(?i)(?:purge_access_logs_after|retention_days\s*=\s*[1-8][0-9]\b)"#),
        severity: "HIGH",
    },

    // -------------------------------------------------------------
    // 6. GLOBAL STANDARDS (PCI-DSS & HIPAA)
    // -------------------------------------------------------------
    RegulatoryRule {
        rule_id: "PCI_PLAINTEXT_PAN",
        domain: LegalDomain::PciDss,
        statutory_reference: "PCI-DSS v4.0 Req 3.4 & 전자금융감독규정 (카드번호 암호화)",
        title: "Plaintext Primary Account Number (Credit Card PAN)",
        requirement_spec: "Primary Account Numbers (PAN) must be encrypted at rest using strong cryptography and never logged.",
        regex_pattern: Some(r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13})\b"),
        severity: "CRITICAL",
    },
    RegulatoryRule {
        rule_id: "HIPAA_EHR_UNENCRYPTED",
        domain: LegalDomain::Hipaa,
        statutory_reference: "HIPAA Security Rule 45 CFR §164.312(a)(2)(iv) (ePHI Encryption)",
        title: "Unencrypted Electronic Protected Health Information (ePHI)",
        requirement_spec: "Electronic Protected Health Information (diagnoses, medical IDs, clinical notes) must be encrypted at rest and in transit.",
        regex_pattern: Some(r#"(?i)(?:patient_icd10|medical_record_number|clinical_diagnosis)\s*:\s*String"#),
        severity: "HIGH",
    },
];
