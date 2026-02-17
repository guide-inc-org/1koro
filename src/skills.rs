use std::path::PathBuf;

use anyhow::Result;

pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

pub struct SkillLoader {
    skills_dir: PathBuf,
}

impl SkillLoader {
    pub fn new(base_dir: &PathBuf) -> Self {
        Self {
            skills_dir: base_dir.join("skills"),
        }
    }

    /// Load skill summaries (name + first non-heading line) for system prompt.
    /// Full skill content is loaded on-demand via read_file tool.
    pub fn load_summaries(&self) -> Result<Vec<SkillSummary>> {
        let mut skills = Vec::new();

        if !self.skills_dir.exists() {
            return Ok(skills);
        }

        for entry in std::fs::read_dir(&self.skills_dir)?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    let content = std::fs::read_to_string(&skill_file)?;
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let description = content
                        .lines()
                        .find(|l| !l.starts_with('#') && !l.trim().is_empty())
                        .unwrap_or("")
                        .to_string();
                    skills.push(SkillSummary {
                        name,
                        description,
                        path: skill_file,
                    });
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }
}
