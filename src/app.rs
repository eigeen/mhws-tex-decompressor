use std::{
    io::{self, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use fs_err as fs;

use color_eyre::eyre::bail;
use dialoguer::{Input, MultiSelect, Select, theme::ColorfulTheme};
use fs::OpenOptions;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use parking_lot::Mutex;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use re_tex::tex::Tex;
use ree_pak_core::{
    filename::{FileNameExt, FileNameTable},
    pak::PakEntry,
    read::archive::PakArchiveReader,
    write::FileOptions,
};

use crate::{chunk::ChunkName, util::human_bytes};

const FILE_NAME_LIST: &[u8] = include_bytes!("../assets/MHWs_STM_Release.list.zst");
const AUTO_CHUNK_SELECTION_SIZE_THRESHOLD: usize = 50 * 1024 * 1024; // 50MB
const FALSE_TRUE_SELECTION: [&str; 2] = ["False", "True"];

struct ChunkSelection {
    chunk_name: ChunkName,
    file_size: u64,
}

impl std::fmt::Display for ChunkSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.chunk_name, human_bytes(self.file_size))?;
        Ok(())
    }
}

#[derive(Default)]
pub struct App {
    filename_table: Option<FileNameTable>,
}

impl App {
    pub fn run(&mut self) -> color_eyre::Result<()> {
        println!("Version v{} - Tool by @Eigeen", env!("CARGO_PKG_VERSION"));
        println!("Get updates at https://github.com/eigeen/mhws-tex-decompressor");
        println!();

        println!("Loading embedded file name table...");
        let filename_table = FileNameTable::from_bytes(FILE_NAME_LIST)?;
        self.filename_table = Some(filename_table);

        // Mode selection
        let mode = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select mode")
            .items(&["Automatic", "Manual"])
            .default(0)
            .interact()?;
        let auto_mode = mode == 0;

        if auto_mode {
            self.auto_mode()
        } else {
            self.manual_mode()
        }
    }

    fn filename_table(&self) -> &FileNameTable {
        self.filename_table.as_ref().unwrap()
    }

    fn process_chunk(
        &self,
        filename_table: &FileNameTable,
        input_path: &Path,
        output_path: &Path,
        use_full_package_mode: bool,
        use_feature_clone: bool,
    ) -> color_eyre::Result<()> {
        println!("Processing chunk: {}", input_path.display());

        let file = fs::File::open(input_path)?;
        let mut reader = io::BufReader::new(file);

        let pak_archive = ree_pak_core::read::read_archive(&mut reader)?;
        let archive_reader = PakArchiveReader::new(reader, &pak_archive);
        let archive_reader_mtx = Mutex::new(archive_reader);

        // filtered entries
        let entries = if use_full_package_mode {
            pak_archive.entries().iter().collect::<Vec<_>>()
        } else {
            println!("Filtering entries...");
            pak_archive
                .entries()
                .iter()
                .filter(|entry| is_tex_file(entry.hash(), filename_table))
                .collect::<Vec<_>>()
        };

        // new pak archive
        let out_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(output_path)?;
        let pak_writer = ree_pak_core::write::PakWriter::new(out_file, entries.len() as u64);
        let pak_writer_mtx = Arc::new(Mutex::new(pak_writer));

        let bar = ProgressBar::new(entries.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("Bytes written: {msg}\n{pos}/{len} {wide_bar}")?,
        );
        bar.enable_steady_tick(Duration::from_millis(200));

        let pak_writer_mtx1 = Arc::clone(&pak_writer_mtx);
        let bar1 = bar.clone();
        let bytes_written = AtomicUsize::new(0);
        let err = entries
            .par_iter()
            .try_for_each(move |&entry| -> color_eyre::Result<()> {
                let pak_writer_mtx = &pak_writer_mtx1;
                let bar = &bar1;
                // read raw tex file
                // parse tex file
                let mut entry_reader = {
                    let mut archive_reader = archive_reader_mtx.lock();
                    archive_reader.owned_entry_reader(entry.clone())?
                };

                if !is_tex_file(entry.hash(), filename_table) {
                    // plain file, just copy
                    let mut buf = vec![];
                    std::io::copy(&mut entry_reader, &mut buf)?;
                    let mut pak_writer = pak_writer_mtx.lock();
                    let write_bytes = write_to_pak(
                        &mut pak_writer,
                        entry,
                        entry.hash(),
                        &buf,
                        use_feature_clone,
                    )?;
                    bytes_written.fetch_add(write_bytes, Ordering::SeqCst);
                } else {
                    let mut tex = Tex::from_reader(&mut entry_reader)?;
                    // decompress mipmaps
                    tex.batch_decompress()?;

                    let tex_bytes = tex.as_bytes()?;
                    let mut pak_writer = pak_writer_mtx.lock();
                    let write_bytes = write_to_pak(
                        &mut pak_writer,
                        entry,
                        entry.hash(),
                        &tex_bytes,
                        use_feature_clone,
                    )?;
                    bytes_written.fetch_add(write_bytes, Ordering::SeqCst);
                }

                bar.inc(1);
                if bar.position() % 100 == 0 {
                    bar.set_message(
                        HumanBytes(bytes_written.load(Ordering::SeqCst) as u64).to_string(),
                    );
                }
                Ok(())
            });
        if let Err(e) = err {
            eprintln!("Error occurred when processing tex: {e}");
            eprintln!(
                "The process terminated early, we'll save the current processed tex files to pak file."
            );
        }

        match Arc::try_unwrap(pak_writer_mtx) {
            Ok(pak_writer) => pak_writer.into_inner().finish()?,
            Err(_) => panic!("Arc::try_unwrap failed"),
        };

        bar.finish();

        Ok(())
    }

    fn auto_mode(&mut self) -> color_eyre::Result<()> {
        let current_dir = std::env::current_dir()?;

        wait_for_enter(
            r#"Check list:

1. Your game is already updated to the latest version.
2. Uninstalled all the mods, or the generated files will break mods.

I'm sure I've checked the list, press Enter to continue"#,
        );

        let game_dir: String = Input::<String>::with_theme(&ColorfulTheme::default())
            .show_default(true)
            .default(current_dir.to_string_lossy().to_string())
            .with_prompt("Input MonsterHunterWilds directory path")
            .interact_text()
            .unwrap()
            .trim_matches(|c| c == '\"' || c == '\'')
            .to_string();

        let game_dir = Path::new(&game_dir);
        if !game_dir.is_dir() {
            bail!("game directory not exists.");
        }

        // scan for pak files
        let dir = fs::read_dir(game_dir)?;
        let mut all_chunks: Vec<ChunkName> = vec![];
        for entry in dir {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.ends_with(".pak") || !file_name.starts_with("re_chunk_") {
                continue;
            }

            let chunk_name = match ChunkName::try_from_str(&file_name) {
                Ok(chunk_name) => chunk_name,
                Err(e) => {
                    println!("Invalid chunk name, skipped: {e}");
                    continue;
                }
            };
            all_chunks.push(chunk_name);
        }
        all_chunks.sort();

        // show chunks for selection
        // only show sub chunks
        let chunk_selections = all_chunks
            .iter()
            .filter_map(|chunk| {
                if chunk.sub_id.is_some() {
                    Some(chunk.to_string())
                } else {
                    None
                }
            })
            .map(|file_name| {
                let file_path = game_dir.join(&file_name);
                let file_size = fs::metadata(file_path)?.len();
                Ok(ChunkSelection {
                    chunk_name: ChunkName::try_from_str(&file_name)?,
                    file_size,
                })
            })
            .collect::<color_eyre::Result<Vec<_>>>()?;
        if chunk_selections.is_empty() {
            bail!("No available pak files found.");
        }

        let selected_chunks: Vec<bool> = chunk_selections
            .iter()
            .map(|chunk_selection| {
                Ok(chunk_selection.file_size >= AUTO_CHUNK_SELECTION_SIZE_THRESHOLD as u64)
            })
            .collect::<color_eyre::Result<Vec<_>>>()?;

        let selected_chunks: Option<Vec<usize>> =
            MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Select chunks to process (Space to select, Enter to confirm)")
                .items(&chunk_selections)
                .defaults(&selected_chunks)
                .interact_opt()?;
        let Some(selected_chunks) = selected_chunks else {
            bail!("No chunks selected.");
        };

        let selected_chunks = selected_chunks
            .iter()
            .map(|i| chunk_selections[*i].chunk_name.clone())
            .collect::<Vec<_>>();

        // replace mode: replace original files with uncompressed files
        // patch mode: generate patch files after original patch files
        let use_replace_mode = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "Replace original files with uncompressed files? (Will automatically backup original files)",
            )
            .default(0)
            .items(&FALSE_TRUE_SELECTION)
            .interact()
            .unwrap();
        let use_replace_mode = use_replace_mode == 1;

        // start processing
        for chunk_name in selected_chunks {
            let chunk_path = game_dir.join(chunk_name.to_string());
            let output_path = if use_replace_mode {
                // In replace mode, first generate a temporary decompressed file
                let temp_path = chunk_path.with_extension("pak.temp");
                // Backup the original file
                let backup_path = chunk_path.with_extension("pak.backup");
                if backup_path.exists() {
                    fs::remove_file(&backup_path)?;
                }
                fs::rename(&chunk_path, &backup_path)?;
                temp_path
            } else {
                // In patch mode
                // Find the max patch id for the current chunk series
                let max_patch_id = all_chunks
                    .iter()
                    .filter(|c| {
                        c.major_id == chunk_name.major_id
                            && c.patch_id == chunk_name.patch_id
                            && c.sub_id == chunk_name.sub_id
                    })
                    .filter_map(|c| c.sub_patch_id)
                    .max()
                    .unwrap_or(0);

                let new_patch_id = max_patch_id + 1;

                // Create a new chunk name
                let mut output_chunk_name = chunk_name.clone();
                output_chunk_name.sub_patch_id = Some(new_patch_id);

                // Add the new patch to the chunk list so it can be found in subsequent processing
                all_chunks.push(output_chunk_name.clone());

                game_dir.join(output_chunk_name.to_string())
            };

            println!("Output patch file: {}", output_path.display());
            self.process_chunk(
                self.filename_table(),
                &chunk_path,
                &output_path,
                use_replace_mode,
                true,
            )?;

            // In replace mode, rename the temporary file to the original file name
            if use_replace_mode {
                fs::rename(&output_path, &chunk_path)?;
            }
            println!();
        }

        Ok(())
    }

    fn manual_mode(&mut self) -> color_eyre::Result<()> {
        let input: String = Input::with_theme(&ColorfulTheme::default())
            .show_default(true)
            .default("re_chunk_000.pak.sub_000.pak".to_string())
            .with_prompt("Input .pak file path")
            .interact_text()
            .unwrap()
            .trim_matches(|c| c == '\"' || c == '\'')
            .to_string();

        let input_path = Path::new(&input);
        if !input_path.is_file() {
            bail!("input file not exists.");
        }

        let use_full_package_mode = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "Package all files, including non-tex files (for replacing original files)",
            )
            .default(0)
            .items(&FALSE_TRUE_SELECTION)
            .interact()
            .unwrap();
        let use_full_package_mode = use_full_package_mode == 1;

        let use_feature_clone = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Clone feature flags from original file?")
            .default(1)
            .items(&FALSE_TRUE_SELECTION)
            .interact()
            .unwrap();
        let use_feature_clone = use_feature_clone == 1;

        self.process_chunk(
            self.filename_table(),
            input_path,
            &input_path.with_extension("uncompressed.pak"),
            use_full_package_mode,
            use_feature_clone,
        )?;

        Ok(())
    }
}

fn is_tex_file(hash: u64, file_name_table: &FileNameTable) -> bool {
    let Some(file_name) = file_name_table.get_file_name(hash) else {
        return false;
    };
    file_name.get_name().ends_with(".tex.241106027")
}

fn write_to_pak<W>(
    writer: &mut ree_pak_core::write::PakWriter<W>,
    entry: &PakEntry,
    file_name: impl FileNameExt,
    data: &[u8],
    use_feature_clone: bool,
) -> color_eyre::Result<usize>
where
    W: io::Write + io::Seek,
{
    let mut file_options = FileOptions::default();
    if use_feature_clone {
        file_options = file_options.with_unk_attr(*entry.unk_attr())
    }
    writer.start_file(file_name, file_options)?;
    writer.write_all(data)?;
    Ok(data.len())
}

fn wait_for_enter(msg: &str) {
    let _: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(msg)
        .allow_empty(true)
        .interact_text()
        .unwrap();
}
