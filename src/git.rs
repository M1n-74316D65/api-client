use anyhow::{Context, Result};
use git2::{Repository, Status, StatusOptions};
use std::path::{Path, PathBuf};

pub struct GitService {
    repo: Repository,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    New,
    Modified,
    Deleted,
    Renamed,
    Typechange,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct GitFileChange {
    pub path: PathBuf,
    pub status: FileStatus,
    pub is_staged: bool,
}

impl GitService {
    pub fn new(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path).context("Failed to open git repository")?;
        Ok(Self { repo })
    }

    pub fn get_status(&self) -> Result<Vec<GitFileChange>> {
        let mut changes = Vec::new();
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        for entry in statuses.iter() {
            let path = PathBuf::from(entry.path().unwrap_or(""));
            let status = entry.status();

            if status.contains(Status::INDEX_NEW)
                || status.contains(Status::INDEX_MODIFIED)
                || status.contains(Status::INDEX_DELETED)
                || status.contains(Status::INDEX_RENAMED)
                || status.contains(Status::INDEX_TYPECHANGE)
            {
                changes.push(GitFileChange {
                    path: path.clone(),
                    status: self.map_status(status),
                    is_staged: true,
                });
            }

            if status.contains(Status::WT_NEW)
                || status.contains(Status::WT_MODIFIED)
                || status.contains(Status::WT_DELETED)
                || status.contains(Status::WT_RENAMED)
                || status.contains(Status::WT_TYPECHANGE)
            {
                changes.push(GitFileChange {
                    path,
                    status: self.map_status(status),
                    is_staged: false,
                });
            }
        }

        Ok(changes)
    }

    fn map_status(&self, status: Status) -> FileStatus {
        if status.contains(Status::INDEX_NEW) || status.contains(Status::WT_NEW) {
            FileStatus::New
        } else if status.contains(Status::INDEX_MODIFIED) || status.contains(Status::WT_MODIFIED) {
            FileStatus::Modified
        } else if status.contains(Status::INDEX_DELETED) || status.contains(Status::WT_DELETED) {
            FileStatus::Deleted
        } else if status.contains(Status::INDEX_RENAMED) || status.contains(Status::WT_RENAMED) {
            FileStatus::Renamed
        } else if status.contains(Status::INDEX_TYPECHANGE)
            || status.contains(Status::WT_TYPECHANGE)
        {
            FileStatus::Typechange
        } else {
            FileStatus::Unknown
        }
    }

    #[allow(dead_code)]
    pub fn stage_file(&self, path: &Path) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_path(path)?;
        index.write()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn unstage_file(&self, path: &Path) -> Result<()> {
        let head = self.repo.head()?.peel_to_commit()?;
        let path_str = path.to_str().context("Invalid path")?;
        self.repo
            .reset_default(Some(&head.as_object()), [path_str])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn commit(&self, message: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        let oid = index.write_tree()?;
        let tree = self.repo.find_tree(oid)?;

        let signature = self.repo.signature()?;
        let parent_commit = self.repo.head()?.peel_to_commit()?;

        self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;

        Ok(())
    }

    pub fn get_current_branch(&self) -> Result<String> {
        let head = self.repo.head()?;
        let name = head.shorthand().unwrap_or("HEAD").to_string();
        Ok(name)
    }
}
