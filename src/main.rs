use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use color_eyre::eyre::bail;
use colored::Colorize;
use dialoguer::{Input, Select, theme::ColorfulTheme};
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

const FILE_NAME_LIST: &[u8] = include_bytes!("../assets/MHWs_STM_Release.list.zst");

fn main() {
    std::panic::set_hook(Box::new(panic_hook));

    println!("Version v{} - Tool by @Eigeen", env!("CARGO_PKG_VERSION"));

    if let Err(e) = main_entry() {
        eprintln!("{}: {:#}", "Error".red().bold(), e);
        wait_for_exit();
        std::process::exit(1);
    }
    wait_for_exit();
}

fn panic_hook(info: &std::panic::PanicHookInfo) {
    eprintln!("{}: {}", "Panic".red().bold(), info);
    wait_for_exit();
    std::process::exit(1);
}

fn main_entry() -> color_eyre::Result<()> {
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

    const FALSE_TRUE_SELECTION: [&str; 2] = ["False", "True"];

    let use_full_package_mode = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Package all files, including non-tex files (for replacing original files)")
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

    println!("Loading embedded file name table...");
    let filename_table = FileNameTable::from_bytes(FILE_NAME_LIST)?;

    let file = fs::File::open(input_path)?;
    let mut reader = io::BufReader::new(file);

    println!("Reading pak archive...");
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
            .filter(|entry| is_tex_file(entry.hash(), &filename_table))
            .collect::<Vec<_>>()
    };

    // new pak archive
    let output_path = input_path.with_extension("uncompressed.pak");
    println!("Output file: {}", output_path.to_string_lossy());
    let out_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output_path)?;
    let pak_writer = ree_pak_core::write::PakWriter::new(out_file, entries.len() as u64);
    let pak_writer_mtx = Arc::new(Mutex::new(pak_writer));

    let bar = ProgressBar::new(entries.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar().template("Bytes written: {msg}\n{pos}/{len} {wide_bar}")?,
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

            if !is_tex_file(entry.hash(), &filename_table) {
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

    let pak_writer = Arc::try_unwrap(pak_writer_mtx);
    match pak_writer {
        Ok(pak_writer) => pak_writer.into_inner().finish()?,
        Err(_) => panic!("Arc::try_unwrap failed"),
    };

    bar.finish();
    println!("{}", "Done!".cyan().bold());
    if !use_full_package_mode {
        println!(
            "You should rename the output file like `re_chunk_000.pak.sub_000.pak.patch_xxx.pak`, or manage it by your favorite mod manager."
        );
    }

    Ok(())
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

fn wait_for_exit() {
    let _: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Press Enter to exit")
        .allow_empty(true)
        .interact_text()
        .unwrap();
}
