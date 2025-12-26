# Agent Selection

## Algorithm

1. **Analyze Context**
   - Current design phase (Discover/Define/Develop/Deliver)
   - Problem domain (technical, UX, business, etc.)
   - Checkpoint type (Alignment/Progress/Completion)

2. **Match Expertise**
   - Select agents whose expertise aligns with current needs
   - Ensure diverse perspectives (avoid overlap)
   - Consider complementary viewpoints

3. **Selection Criteria**
   | Context | Recommended Agents |
   |---------|-------------------|
   | Technical architecture | Architect, DevOps, Security |
   | User experience | UX Designer, Product Manager, User Researcher |
   | Business strategy | Product Manager, Business Analyst, Stakeholder |
   | Implementation | Senior Dev, QA Engineer, DevOps |

4. **Constraints**
   - Minimum: 2 agents
   - Maximum: 3 agents
   - Must include at least one domain expert

## Output Format
```
**Selected Agents:**
1. [Agent Name] - [Reason for selection]
2. [Agent Name] - [Reason for selection]
3. [Agent Name] - [Reason for selection]
```
