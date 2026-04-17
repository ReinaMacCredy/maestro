# Classification Data

## Project Types

Detect the project type from codebase signals, user description, and tech stack. Match the FIRST type whose detection signals appear. Fall back to `general` when no signals match.

| Type | Detection Signals | Key Discovery Questions | Focus Areas |
|------|------------------|------------------------|-------------|
| api_backend | REST, GraphQL, endpoints, microservices, server | What clients consume this API? What data does it manage? Authentication model? | API design, versioning, rate limiting, documentation |
| mobile_app | iOS, Android, native, React Native, Flutter | Which platforms? Offline support needed? Device capabilities used? | Platform constraints, offline sync, app store requirements |
| saas_b2b | multi-tenant, subscription, enterprise, dashboard | How many tenant tiers? What's the pricing model? Data isolation requirements? | Multi-tenancy, billing integration, admin portals, SSO |
| developer_tool | SDK, library, package, framework, tooling | Who's the target developer? What languages/ecosystems? Extension model? | API ergonomics, documentation, backward compatibility |
| cli_tool | terminal, command-line, flags, arguments, shell | What workflows does it support? Interactive or scriptable? Configuration model? | UX in terminal, help system, shell integration, piping |
| web_app | browser, frontend, SPA, pages, responsive | What browsers/devices? SEO requirements? Real-time features? | Accessibility, performance, responsive design, SEO |
| desktop_app | native, Electron, Tauri, window, system tray | Which OSes? System integration depth? Auto-update strategy? | Platform APIs, distribution, updates, system integration |
| iot_embedded | sensor, device, firmware, protocol, telemetry | What hardware? Communication protocols? Power constraints? | Resource constraints, reliability, OTA updates |
| blockchain_web3 | smart contract, wallet, token, chain, decentralized | Which chain(s)? Token economics? Gas optimization needed? | Security audits, gas efficiency, wallet UX |
| game | engine, player, multiplayer, assets, rendering | Multiplayer? Target platforms? Performance budget? | Game loop, networking, asset pipeline, physics |
| general | (none -- fallback when no other type matches) | What's the core functionality? Who uses it? How is it deployed? | Depends on answers |

## Domain Complexity

Identify the domain from the user's description and project context. Domain determines compliance requirements and risk posture. Default to `general` when no specific domain applies.

| Domain | Complexity | Key Concerns | Compliance Requirements |
|--------|-----------|--------------|------------------------|
| healthcare | high | Patient safety, data privacy, audit trails, interoperability | HIPAA, HL7/FHIR, FDA (if SaMD), SOC 2 |
| fintech | high | Transaction integrity, fraud prevention, audit trails, encryption | PCI-DSS, SOX, KYC/AML, state/federal banking regs |
| govtech | high | Accessibility, data sovereignty, audit trails, security clearance | FedRAMP, Section 508, ITAR, FISMA |
| edtech | medium | Student privacy, content accessibility, LMS integration | FERPA, COPPA (if under 13), WCAG 2.1 |
| aerospace | high | Safety-critical, real-time, certification, redundancy | DO-178C, AS9100, ITAR |
| automotive | high | Safety-critical, real-time, OTA updates, V2X | ISO 26262, ASPICE, UNECE WP.29 |
| scientific | medium | Data reproducibility, precision, large datasets, peer review | Domain-specific (GxP, 21 CFR Part 11 if pharma) |
| legaltech | medium | Document integrity, privilege handling, jurisdiction rules | Bar association rules, e-discovery standards, GDPR |
| insuretech | medium | Actuarial accuracy, claims integrity, underwriting rules | State insurance regs, NAIC model laws, SOC 2 |
| energy | high | Grid reliability, safety systems, SCADA security | NERC CIP, IEC 62443, DOE regulations |
| process_control | high | Real-time, safety interlocks, fault tolerance | IEC 61508/61511, OSHA PSM |
| building_automation | medium | Occupant safety, energy efficiency, system integration | Local building codes, ASHRAE, BACnet standards |
| gaming | low | Content rating, loot box regulations (jurisdiction-dependent) | ESRB/PEGI, gambling regs (if applicable) |
| general | low | Standard security practices, basic data protection | GDPR/CCPA (if PII), SOC 2 (if SaaS) |
| ecommerce | medium | Payment security, inventory accuracy, tax compliance | PCI-DSS, sales tax nexus, consumer protection laws |
