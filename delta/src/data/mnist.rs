//! BSD 3-Clause License
//!
//! Copyright (c) 2024, Marcus Cvjeticanin
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice, this
//!    list of conditions and the following disclaimer.
//!
//! 2. Redistributions in binary form must reproduce the above copyright notice,
//!    this list of conditions and the following disclaimer in the documentation
//!    and/or other materials provided with the distribution.
//!
//! 3. Neither the name of the copyright holder nor the names of its
//!    contributors may be used to endorse or promote products derived from
//!    this software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//! DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
//! FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//! DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//! SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//! CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
//! OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//! OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::common::{Dataset, DatasetOps};
use crate::common::tensor_ops::Tensor;
use flate2::read::GzDecoder;
use rand::seq::SliceRandom;
use reqwest;
use std::fs::File;
use std::future::Future;
use std::io::{self, Read};
use std::pin::Pin;

/// A struct representing the MNIST dataset.
pub struct MnistDataset {
    train: Option<Dataset>,
    test: Option<Dataset>,
}

impl MnistDataset {
    const MNIST_URL: &'static str = "https://storage.googleapis.com/cvdf-datasets/mnist";
    const MNIST_TRAIN_DATA_FILENAME: &'static str = "train-images-idx3-ubyte.gz";
    const MNIST_TRAIN_LABELS_FILENAME: &'static str = "train-labels-idx1-ubyte.gz";
    const MNIST_TEST_DATA_FILENAME: &'static str = "t10k-images-idx3-ubyte.gz";
    const MNIST_TEST_LABELS_FILENAME: &'static str = "t10k-labels-idx1-ubyte.gz";
    const MNIST_IMAGE_SIZE: usize = 28;
    const MNIST_NUM_CLASSES: usize = 10;
    const TRAIN_EXAMPLES: usize = 60_000;
    const TEST_EXAMPLES: usize = 10_000;

    /// Load the MNIST dataset
    ///
    /// # Arguments
    /// * `is_train` - Whether to load the training or testing dataset
    ///
    /// # Returns
    /// A dataset containing the MNIST data
    async fn load_data(is_train: bool) -> Dataset {
        let (data_filename, labels_filename, num_examples) = if is_train {
            (
                Self::MNIST_TRAIN_DATA_FILENAME,
                Self::MNIST_TRAIN_LABELS_FILENAME,
                Self::TRAIN_EXAMPLES,
            )
        } else {
            (
                Self::MNIST_TEST_DATA_FILENAME,
                Self::MNIST_TEST_LABELS_FILENAME,
                Self::TEST_EXAMPLES,
            )
        };

        let data_bytes = Self::get_bytes_data(data_filename).await;
        let labels_bytes = Self::get_bytes_data(labels_filename).await;

        let data = Self::parse_images(&data_bytes, num_examples);
        let labels = Self::parse_labels(&labels_bytes, num_examples);

        Dataset::new(data, labels)
    }

    /// Parse the images from the MNIST dataset
    ///
    /// # Arguments
    /// * `data` - The compressed data bytes
    /// * `num_images` - The number of images to parse
    ///
    /// # Returns
    /// A tensor containing the parsed images
    fn parse_images(data: &[u8], num_images: usize) -> Tensor {
        let image_data = &data[16..]; // Skip the 16-byte header
        let num_pixels = Self::MNIST_IMAGE_SIZE * Self::MNIST_IMAGE_SIZE;
        let mut tensor_data = vec![0.0; num_images * num_pixels];

        for i in 0..num_images {
            let start = i * num_pixels;
            let end = start + num_pixels;
            for (j, &pixel) in image_data[start..end].iter().enumerate() {
                tensor_data[i * num_pixels + j] = pixel as f32 / 255.0; // Normalize to [0, 1]
            }
        }

        Tensor::new(
            tensor_data,
            vec![
                num_images,
                Self::MNIST_IMAGE_SIZE,
                Self::MNIST_IMAGE_SIZE,
                1,
            ],
        )
    }

    /// Parse the labels from the MNIST dataset
    ///
    /// # Arguments
    /// * `data` - The compressed data bytes
    /// * `num_labels` - The number of labels to parse
    ///
    /// # Returns
    /// A tensor containing the parsed labels
    fn parse_labels(data: &[u8], num_labels: usize) -> Tensor {
        let label_data = &data[8..]; // Skip the 8-byte header
        let mut tensor_data = vec![0.0; num_labels * Self::MNIST_NUM_CLASSES];

        for (i, &label) in label_data.iter().enumerate() {
            tensor_data[i * Self::MNIST_NUM_CLASSES + label as usize] = 1.0; // One-hot encoding
        }

        Tensor::new(tensor_data, vec![num_labels, Self::MNIST_NUM_CLASSES])
    }

    /// Download and decompress a file from the MNIST dataset
    ///
    /// # Arguments
    /// * `filename` - The name of the file to download
    ///
    /// # Returns
    /// A vector of bytes containing the decompressed data
    async fn get_bytes_data(filename: &str) -> Vec<u8> {
        let file_path = format!(".cache/data/mnist/{}", filename);
        if std::path::Path::new(&file_path).exists() {
            return Self::decompress_gz(&file_path).unwrap();
        }

        let url = format!("{}/{}", Self::MNIST_URL, filename);
        println!("Downloading MNIST dataset from {}", &url);

        let compressed_data = reqwest::get(&url)
            .await
            .expect("Failed to download data")
            .bytes()
            .await
            .expect("Failed to read data")
            .to_vec();

        std::fs::create_dir_all(".cache/data/mnist").unwrap();
        std::fs::write(&file_path, &compressed_data).unwrap();

        Self::decompress_gz(&file_path).unwrap()
    }

    /// Decompress a gzip file
    ///
    /// # Arguments
    /// * `file_path` - The path to the gzip file
    ///
    /// # Returns
    /// A vector of bytes containing the decompressed data
    fn decompress_gz(file_path: &str) -> io::Result<Vec<u8>> {
        let file = File::open(file_path)?;
        let mut decoder = GzDecoder::new(file);
        let mut decompressed_data = Vec::new();
        decoder.read_to_end(&mut decompressed_data)?;
        println!("Unarchived file: {}", file_path);
        Ok(decompressed_data)
    }
}

impl DatasetOps for MnistDataset {
    type LoadFuture = Pin<Box<dyn Future<Output = Self> + Send>>;

    /// Load the MNIST dataset
    ///
    /// # Returns
    /// A dataset containing the MNIST data
    fn load_train() -> Self::LoadFuture {
        Box::pin(async {
            Self {
                train: Some(Self::load_data(true).await),
                test: None,
            }
        })
    }

    /// Load the MNIST dataset
    ///
    /// # Returns
    /// A dataset containing the MNIST data
    fn load_test() -> Self::LoadFuture {
        Box::pin(async {
            Self {
                train: None,
                test: Some(Self::load_data(false).await),
            }
        })
    }

    /// Get the number of examples in the dataset
    ///
    /// # Returns
    /// The number of examples in the dataset
    fn len(&self) -> usize {
        if let Some(ref train) = self.train {
            train.inputs.data.shape()[0]
        } else if let Some(ref test) = self.test {
            test.inputs.data.shape()[0]
        } else {
            0
        }
    }

    /// Normalizes the dataset.
    ///
    /// # Arguments
    /// * `min` - The minimum value for normalization.
    /// * `max` - The maximum value for normalization.
    fn normalize(&mut self, min: f32, max: f32) {
        let _ = max;
        let _ = min;
        todo!()
    }

    /// Adds noise to the dataset.
    ///
    /// # Arguments
    /// * `noise_level` - The level of noise to add.
    fn add_noise(&mut self, noise_level: f32) {
        let _ = noise_level;
        todo!()
    }

    /// Get a batch of data from the dataset
    ///
    /// # Arguments
    /// * `batch_idx` - The index of the batch to get
    /// * `batch_size` - The size of the batch to get
    ///
    /// # Returns
    /// A tuple containing the input and label tensors for the batch
    fn get_batch(&self, batch_idx: usize, batch_size: usize) -> (Tensor, Tensor) {
        // Determine which dataset to use: train or test
        let dataset = match (self.train.as_ref(), self.test.as_ref()) {
            (Some(train), _) => train,          // Use the train dataset if available
            (_, Some(test)) => test,            // Otherwise, use the test dataset
            _ => panic!("Dataset not loaded!"), // Panic if neither dataset is loaded
        };

        // Get the total number of samples in the dataset
        let total_samples = dataset.inputs.shape()[0];

        // Calculate the start and end indices for the batch
        let start_idx = batch_idx * batch_size;
        let end_idx = start_idx + batch_size;

        // Ensure the start index is within range
        if start_idx >= total_samples {
            panic!(
                "Batch index {} out of range. Total samples: {}",
                batch_idx, total_samples
            );
        }

        // Adjust the end index if it exceeds the total samples
        let adjusted_end_idx = end_idx.min(total_samples);

        // Slice the input tensor for the batch
        let inputs_batch = dataset.inputs.slice(vec![
            start_idx..adjusted_end_idx, // Batch range along the sample dimension
            0..28,                       // Full range for the image height
            0..28,                       // Full range for the image width
            0..1,                        // Full range for the channels (grayscale)
        ]);

        // Slice the label tensor for the batch
        let labels_batch = dataset.labels.slice(vec![
            start_idx..adjusted_end_idx, // Batch range along the sample dimension
            0..10,                       // Full range for the classes (one-hot encoding)
        ]);

        // Return the inputs and labels for the batch
        (inputs_batch, labels_batch)
    }

    /// Calculates the loss between the predicted outputs and the true targets.
    ///
    /// # Arguments
    ///
    /// * `outputs` - The predicted outputs from the model (logits or probabilities).
    /// * `targets` - The true target values (one-hot encoded).
    ///
    /// # Returns
    ///
    /// The calculated loss as a `f32` value.
    fn loss(&self, outputs: &Tensor, targets: &Tensor) -> f32 {
        let outputs_data = outputs.data.clone();
        let targets_data = targets.data.clone();

        let batch_size = targets.shape()[0];
        let num_classes = targets.shape()[1];

        let mut loss = 0.0;

        for i in 0..batch_size {
            for j in 0..num_classes {
                let target = targets_data[i * num_classes + j];
                let predicted = outputs_data[i * num_classes + j].max(1e-15); // Avoid log(0)
                loss -= target * predicted.ln(); // Cross-entropy loss
            }
        }

        loss / batch_size as f32
    }

    /// Calculates the gradient of the loss with respect to the predicted outputs.
    ///
    /// # Arguments
    ///
    /// * `outputs` - The predicted outputs from the model (probabilities).
    /// * `targets` - The true target values (one-hot encoded).
    ///
    /// # Returns
    ///
    /// A `Tensor` containing the gradients of the loss with respect to the outputs.
    fn loss_grad(&self, outputs: &Tensor, targets: &Tensor) -> Tensor {
        let outputs_data = outputs.data.iter().cloned().collect::<Vec<f32>>();
        let targets_data = targets.data.iter().cloned().collect::<Vec<f32>>();

        let batch_size = targets.shape()[0];
        let num_classes = targets.shape()[1];
        assert_eq!(
            outputs.shape(),
            targets.shape(),
            "Outputs and targets must have the same shape"
        );

        let mut grad_data = vec![0.0; batch_size * num_classes];

        for i in 0..batch_size {
            for j in 0..num_classes {
                let target = targets_data[i * num_classes + j];
                let predicted = outputs_data[i * num_classes + j];
                grad_data[i * num_classes + j] = (predicted - target) / batch_size as f32;
            }
        }

        Tensor::new(grad_data, outputs.shape().clone())
    }

    /// Shuffle the dataset
    fn shuffle(&mut self) {
        if let Some(dataset) = &mut self.train {
            // Retrieve the number of samples in the dataset
            let num_samples = dataset.inputs.shape()[0];

            // Create an index permutation
            let mut indices: Vec<usize> = (0..num_samples).collect();
            let mut rng = rand::thread_rng();
            indices.shuffle(&mut rng);

            // Create new tensors with shuffled data
            let shuffled_inputs = dataset.inputs.permute(indices.clone());
            let shuffled_labels = dataset.labels.permute(indices);

            // Update the dataset with the shuffled tensors
            dataset.inputs = shuffled_inputs;
            dataset.labels = shuffled_labels;
        }

        if let Some(dataset) = &mut self.test {
            // Retrieve the number of samples in the dataset
            let num_samples = dataset.inputs.shape()[0];

            // Create an index permutation
            let mut indices: Vec<usize> = (0..num_samples).collect();
            let mut rng = rand::thread_rng();
            indices.shuffle(&mut rng);

            // Create new tensors with shuffled data
            let shuffled_inputs = dataset.inputs.permute(indices.clone());
            let shuffled_labels = dataset.labels.permute(indices);

            // Update the dataset with the shuffled tensors
            dataset.inputs = shuffled_inputs;
            dataset.labels = shuffled_labels;
        }
    }

    fn clone(&self) -> Self {
        Self {
            train: self.train.clone(),
            test: self.test.clone(),
        }
    }
}